use evolution_fixture::{Message, MessageView, ViewEncode};

#[test]
fn packed_child_values_fit_the_schema_aware_completion_reservation() {
    use evolution_fixture::{v1, v2, DecodeOptions};
    let mut wire = vec![0xc0, 0x3e, 7, 0x3a, 0x84, 4, 0x82, 2, 0x80, 4];
    wire.extend(std::iter::repeat_n(0, 512));
    let expected = v2::Record::decode_from_slice(&wire).unwrap();
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options.decode_from_slice::<v1::Record>(&wire).unwrap();
    let view = options.decode_view::<v1::RecordView<'_>>(&wire).unwrap();
    for bytes in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
        assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
    }
}

#[test]
fn pending_singular_snapshot_covers_expanding_packed_enum_values() {
    use evolution_fixture::{v1, v2, DecodeOptions};
    let mut wire = vec![0xc0, 0x3e, 7, 0x18, 0, 0x12, 0x80, 0x0a];
    // 256 five-byte int32 encodings become ten-byte canonical negative values.
    for _ in 0..256 {
        wire.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x0f]);
    }
    wire.extend_from_slice(&[0x18, 1]);
    let expected = v2::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options.decode_from_slice::<v1::RepeatedRecord>(&wire).unwrap();
    let view = options.decode_view::<v1::RepeatedRecordView<'_>>(&wire).unwrap();
    for output in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
        assert_eq!(v2::RepeatedRecord::decode_from_slice(&output).unwrap(), expected);
    }
}

#[test]
fn event_budget_failure_keeps_the_completed_later_known_occurrence() {
    use evolution_fixture::{v1, v2, DecodeContext};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 1, 0x10, 2];
    for allowance in 0..600 {
        let unknown = core::cell::Cell::new(usize::MAX);
        let budget = core::cell::Cell::new(allowance);
        let mut owned = v1::EnumRecord::decode_from_slice(&first).unwrap();
        let _ = owned.merge_to_limit(&mut &later[..], DecodeContext::new(100, &unknown).with_element_memory(&budget), 0);
        if owned.mode == Some(v1::record::Mode::OTHER) {
            assert_eq!(v2::EnumRecord::decode_from_slice(&owned.encode_to_vec()).unwrap().mode, Some(v2::record::Mode::OTHER), "owned budget {allowance}");
        }
        let budget = core::cell::Cell::new(allowance);
        let mut view = v1::EnumRecordView::decode_view(&first).unwrap();
        let _ = view.merge_into_view(&later, DecodeContext::new(100, &unknown).with_element_memory(&budget));
        if view.mode == Some(v1::record::Mode::OTHER) {
            assert_eq!(v2::EnumRecord::decode_from_slice(&view.encode_to_vec()).unwrap().mode, Some(v2::record::Mode::OTHER), "view budget {allowance}");
        }
    }
}

#[test]
fn every_budget_boundary_keeps_materialized_future_enum_last() {
    use evolution_fixture::{v1, v2, DecodeContext};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 1];
    for allowance in 0..400 {
        let unknown = core::cell::Cell::new(usize::MAX);
        let budget = core::cell::Cell::new(allowance);
        let mut owned = v1::EnumRecord::decode_from_slice(&first).unwrap();
        let _ = owned.merge_to_limit(&mut &later[..], DecodeContext::new(100, &unknown).with_element_memory(&budget), 0);
        if owned.__buffa_unknown_fields.len() == 2 {
            assert_eq!(v2::EnumRecord::decode_from_slice(&owned.encode_to_vec()).unwrap().mode, Some(v2::record::Mode::NEW), "owned budget {allowance}");
        }
        let budget = core::cell::Cell::new(allowance);
        let mut view = v1::EnumRecordView::decode_view(&first).unwrap();
        let _ = view.merge_into_view(&later, DecodeContext::new(100, &unknown).with_element_memory(&budget));
        if view.__buffa_unknown_fields.len() == 2 {
            for bytes in [view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
                assert_eq!(v2::EnumRecord::decode_from_slice(&bytes).unwrap().mode, Some(v2::record::Mode::NEW), "view budget {allowance}");
            }
        }
    }
}

#[test]
fn failed_final_baseline_budget_keeps_owned_enum_winner() {
    use evolution_fixture::{v1, v2, DecodeContext};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 1];
    let mut measured = v1::EnumRecord::decode_from_slice(&first).unwrap();
    let unknown = core::cell::Cell::new(usize::MAX);
    let budget = core::cell::Cell::new(1024);
    measured.merge_to_limit(&mut &later[..], DecodeContext::new(100, &unknown).with_element_memory(&budget), 0).unwrap();
    let used = 1024 - budget.get();
    let mut retained = v1::EnumRecord::decode_from_slice(&first).unwrap();
    let budget = core::cell::Cell::new(used - 1);
    assert!(retained.merge_to_limit(&mut &later[..], DecodeContext::new(100, &unknown).with_element_memory(&budget), 0).is_err());
    let restored = v2::EnumRecord::decode_from_slice(&retained.encode_to_vec()).unwrap();
    // Prepayment may reject the final occurrence before it is materialized.
    let expected = if retained.__buffa_unknown_fields.len() == 2 { v2::record::Mode::NEW } else { v2::record::Mode::READY };
    assert_eq!(restored.mode, Some(expected));
    retained.merge_from_slice(&[0x10, 2]).unwrap();
    assert_eq!(v2::EnumRecord::decode_from_slice(&retained.encode_to_vec()).unwrap().mode, Some(v2::record::Mode::OTHER));
}

#[test]
fn failed_final_baseline_budget_keeps_view_enum_winner() {
    use evolution_fixture::{v1, v2, DecodeContext};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 1];
    let mut measured = v1::EnumRecordView::decode_view(&first).unwrap();
    let unknown = core::cell::Cell::new(usize::MAX);
    let budget = core::cell::Cell::new(1024);
    measured.merge_into_view(&later, DecodeContext::new(100, &unknown).with_element_memory(&budget)).unwrap();
    let used = 1024 - budget.get();
    let mut retained = v1::EnumRecordView::decode_view(&first).unwrap();
    let budget = core::cell::Cell::new(used - 1);
    assert!(retained.merge_into_view(&later, DecodeContext::new(100, &unknown).with_element_memory(&budget)).is_err());
    let restored = v2::EnumRecord::decode_from_slice(&retained.encode_to_vec()).unwrap();
    let expected = if retained.__buffa_unknown_fields.len() == 2 { v2::record::Mode::NEW } else { v2::record::Mode::READY };
    assert_eq!(restored.mode, Some(expected));
    let owned = retained.to_owned_message().unwrap();
    assert_eq!(v2::EnumRecord::decode_from_slice(&owned.encode_to_vec()).unwrap().mode, Some(expected));
    retained.merge_into_view(&[0x10, 2], DecodeContext::new(100, &unknown)).unwrap();
    assert_eq!(v2::EnumRecord::decode_from_slice(&retained.encode_to_vec()).unwrap().mode, Some(v2::record::Mode::OTHER));
}

#[test]
fn fragmented_oneof_baseline_is_built_once_per_batch() {
    use evolution_fixture::{v1, DecodeOptions};
    let mut wire = vec![0xc0, 0x3e, 7];
    for _ in 0..512 {
        wire.extend_from_slice(&[0x3a, 2, 0x18, 0]);
    }
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options.decode_from_slice::<v1::Record>(&wire).unwrap();
    let view = options.decode_view::<v1::RecordView<'_>>(&wire).unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
    let Some(v1::record::Choice::Detail(child)) = &owned.choice else { panic!("detail") };
    assert_eq!(child.values.len(), 512);

    let mut edited = owned.clone();
    let Some(v1::record::Choice::Detail(child)) = &mut edited.choice else { panic!("detail") };
    child.left = Some(9);
    let output = edited.encode_to_vec();
    let restored = v1::Record::decode_from_slice(&output).unwrap();
    let Some(v1::record::Choice::Detail(child)) = &restored.choice else { panic!("detail") };
    assert_eq!(child.left, Some(9));
    assert_eq!(child.values.len(), 512);
}

#[test]
fn unknown_only_batch_does_not_recopy_a_large_unchanged_baseline() {
    use evolution_fixture::{v1, DecodeOptions};
    let mut record = v1::Record::default();
    let length = if cfg!(miri) { 4 * 1024 } else { 60 * 1024 };
    record.choice = Some(v1::record::Choice::Text("x".repeat(length)));
    let mut wire = record.encode_to_vec();
    wire.extend_from_slice(&[0xc0, 0x3e, 7]);
    let options = DecodeOptions::new().with_element_memory_limit(4 * length + 1024);
    let owned = options.decode_from_slice::<v1::Record>(&wire).unwrap();
    let view = options.decode_view::<v1::RecordView<'_>>(&wire).unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.encode_to_vec(), wire);
}

#[test]
fn repeated_nested_message_occurrences_merge_existing_fields() {
    use evolution_fixture::v1;
    // Separate occurrences of child set separate fields of the same message.
    let wire = [0x32, 2, 0x08, 1, 0x32, 2, 0x10, 2];
    let record = v1::Record::decode_from_slice(&wire).unwrap();
    assert_eq!(record.child.left, Some(1));
    assert_eq!(record.child.right, Some(2));
    let view = v1::RecordView::decode_view(&wire).unwrap();
    assert_eq!(record, view.to_owned_message().unwrap());
}

#[test]
fn failed_fragment_batch_preserves_completed_occurrences() {
    use evolution_fixture::{v1, DecodeContext};
    let first = [0xc0, 0x3e, 7, 0x3a, 2, 0x08, 1];
    let later = [0x3a, 2, 0x10, 2, 0x3a, 3, 0x08];
    let expected = [&first[..], &later[..4]].concat();
    let mut owned = v1::Record::decode_from_slice(&first).unwrap();
    assert!(owned.merge_from_slice(&later).is_err());
    assert_eq!(owned.encode_to_vec(), expected);
    let mut view = v1::RecordView::decode_view(&first).unwrap();
    let limit = core::cell::Cell::new(usize::MAX);
    assert!(view.merge_into_view(&later, DecodeContext::new(100, &limit)).is_err());
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}

#[test]
fn partial_nested_failure_keeps_edits_and_retries_without_losing_history() {
    use evolution_fixture::{v1, DecodeContext};
    let first = [0xc0, 0x3e, 7, 0x3a, 2, 0x08, 1];
    let later = [0x3a, 2, 0x10, 2, 0x3a, 4, 0x08, 3, 0xc0, 0x3e];
    let expected = [0xc0, 0x3e, 7, 0x3a, 4, 0x08, 3, 0x10, 2];
    let retry = [0x3a, 2, 0x08, 4];
    let unknown = core::cell::Cell::new(usize::MAX);
    let ctx = DecodeContext::new(100, &unknown);
    let mut owned = v1::Record::decode_from_slice(&first).unwrap();
    assert!(owned.merge_to_limit(&mut &later[..], ctx, 0).is_err());
    assert_eq!(owned.encode_to_vec(), expected);
    let budget = core::cell::Cell::new(0);
    assert!(owned.merge_to_limit(&mut &retry[..], ctx.with_element_memory(&budget), 0).is_err());
    assert_eq!(owned.encode_to_vec(), expected);
    let budget = core::cell::Cell::new(128 * 1024);
    owned.merge_to_limit(&mut &retry[..], ctx.with_element_memory(&budget), 0).unwrap();
    let restored = v1::Record::decode_from_slice(&owned.encode_to_vec()).unwrap();
    let Some(v1::record::Choice::Detail(child)) = restored.choice else { panic!("detail") };
    assert_eq!(child.left, Some(4));
    assert_eq!(child.right, Some(2));

    let mut view = v1::RecordView::decode_view(&first).unwrap();
    assert!(view.merge_into_view(&later, ctx).is_err());
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
    let budget = core::cell::Cell::new(0);
    assert!(view.merge_into_view(&retry, ctx.with_element_memory(&budget)).is_err());
    assert_eq!(view.encode_to_vec(), expected);
    let budget = core::cell::Cell::new(128 * 1024);
    view.merge_into_view(&retry, ctx.with_element_memory(&budget)).unwrap();
    let restored = view.to_owned_message().unwrap();
    let Some(v1::record::Choice::Detail(child)) = restored.choice else { panic!("detail") };
    assert_eq!(child.left, Some(4));
    assert_eq!(child.right, Some(2));
}

#[test]
fn enum_only_projection_preserves_each_winner_across_owners() {
    use evolution_fixture::{v1, v2};
    for (wire, expected) in [
        (vec![0x10, 0, 0x10, 1], v2::record::Mode::NEW),
        (vec![0x10, 1, 0x10, 0], v2::record::Mode::READY),
    ] {
        let owned = v1::EnumRecord::decode_from_slice(&wire).unwrap();
        let view = v1::EnumRecordView::decode_view(&wire).unwrap();
        let handle =
            v1::EnumRecordOwnedView::decode(evolution_fixture::bytes::Bytes::from(wire.clone()))
                .unwrap();
        let cloned = handle.clone();
        drop(handle);
        for output in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            view.to_owned_message().unwrap().encode_to_vec(),
            cloned.to_owned_message().encode_to_vec(),
        ] {
            assert_eq!(
                v2::EnumRecord::decode_from_slice(&output).unwrap().mode,
                Some(expected)
            );
        }
    }
}

#[test]
fn enum_only_projection_keeps_group_edits_separate() {
    use evolution_fixture::{v1, v2};
    let wire = [0x10, 0, 0x18, 1, 0x10, 1, 0x18, 0];
    let original = v1::EnumRecord::decode_from_slice(&wire).unwrap();
    let check = |bytes: Vec<u8>, mode, other| {
        let decoded = v2::EnumRecord::decode_from_slice(&bytes).unwrap();
        assert_eq!((decoded.mode, decoded.other), (Some(mode), Some(other)));
    };
    let mut unrelated = original.clone();
    unrelated.label = Some(42);
    check(
        unrelated.encode_to_vec(),
        v2::record::Mode::NEW,
        v2::record::Mode::READY,
    );
    let mut edited = original.clone();
    edited.other = Some(v1::record::Mode::OTHER);
    check(
        edited.encode_to_vec(),
        v2::record::Mode::NEW,
        v2::record::Mode::OTHER,
    );
    let forced = original.clone().with_mode(v1::record::Mode::READY);
    check(
        forced.encode_to_vec(),
        v2::record::Mode::READY,
        v2::record::Mode::READY,
    );
    let mut cleared = original;
    cleared.mode = None;
    check(
        cleared.encode_to_vec(),
        v2::record::Mode::NEW,
        v2::record::Mode::READY,
    );
    let view = v1::EnumRecordView::decode_view(&wire)
        .unwrap()
        .with_mode(v1::record::Mode::NEGATIVE);
    check(
        view.encode_to_vec(),
        v2::record::Mode::NEGATIVE,
        v2::record::Mode::READY,
    );
    check(
        view.to_owned_message().unwrap().encode_to_vec(),
        v2::record::Mode::NEGATIVE,
        v2::record::Mode::READY,
    );
}

macro_rules! consumer {
    ($test:ident, $version:ident) => {
        #[test]
        fn $test() {
            use evolution_fixture::$version as api;
            let mut record = api::Record::default().with_name("fixture");
            record.mode = Some(api::record::Mode::READY);
            record.choice = Some(api::record::Choice::Text("sample".into()));
            match record.mode.unwrap() {
                api::record::Mode::READY => {}
                _ => panic!("unexpected known mode"),
            }
            let bytes = record.encode_to_vec();
            let view = api::RecordView::decode_view(&bytes).unwrap();
            assert_eq!(view.name, Some("fixture"));
            let handle = api::RecordOwnedView::from_owned(&record).unwrap();
            assert_eq!(handle.view().name, Some("fixture"));
            assert_eq!(handle.to_owned_message().encode_to_vec(), bytes);
            match view.choice.as_ref().unwrap() {
                api::record::ChoiceView::Text(text) => {
                    assert_eq!(*text, "sample")
                }
                _ => panic!("unexpected choice"),
            }
            let json = serde_json::to_value(&record).unwrap();
            assert_eq!(json["name"], "fixture");
            assert_eq!(json["mode"], "READY");
            assert!(json.get("displayName").is_none());
            assert!(
                serde_json::to_value(&record)
                    .unwrap()
                    .get("__buffa_unknown_fields")
                    .is_none()
            );
            let unknown = [0x10, 99, 0xa0, 0x06, 7];
            let record = api::Record::decode_from_slice(&unknown).unwrap();
            assert_eq!(record.encode_to_vec(), unknown);
            let view = api::RecordView::decode_view(&unknown).unwrap();
            assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), unknown);
            let handle = api::RecordOwnedView::decode(unknown.to_vec().into()).unwrap();
            assert_eq!(handle.to_owned_message().encode_to_vec(), unknown);
        }
    };
}
consumer!(old_schema, v1);
consumer!(new_schema, v2);

#[test]
fn old_representation_retains_occurrence_order() {
    use evolution_fixture::{v1, v2};
    for (first, second) in [
        (vec![0x10, 0, 0x10, 1], vec![0x10, 1, 0x10, 0]),
        (
            vec![0x1a, 1, b'a', 0x2a, 1, b'b'],
            vec![0x2a, 1, b'b', 0x1a, 1, b'a'],
        ),
    ] {
        assert_ne!(
            v1::Record::decode_from_slice(&first).unwrap(),
            v1::Record::decode_from_slice(&second).unwrap()
        );
        assert_ne!(
            v2::Record::decode_from_slice(&first).unwrap(),
            v2::Record::decode_from_slice(&second).unwrap()
        );
    }
}

#[test]
fn untouched_mixed_enum_occurrences_keep_their_winner() {
    use evolution_fixture::{v1, v2};
    for wire in [[0x10, 0, 0x10, 1], [0x10, 1, 0x10, 0]] {
        let expected = v2::Record::decode_from_slice(&wire).unwrap();
        let owned = v1::Record::decode_from_slice(&wire).unwrap();
        let view = v1::RecordView::decode_view(&wire).unwrap();
        let handle = v1::RecordOwnedView::decode(wire.to_vec().into()).unwrap();
        for roundtrip in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            handle.to_owned_message().encode_to_vec(),
        ] {
            assert_eq!(v2::Record::decode_from_slice(&roundtrip).unwrap(), expected);
        }
    }
}

#[test]
fn untouched_mixed_oneof_occurrences_keep_their_winner() {
    use evolution_fixture::{v1, v2};
    for wire in [
        [0x1a, 1, b'a', 0x2a, 1, b'b'],
        [0x2a, 1, b'b', 0x1a, 1, b'a'],
    ] {
        let expected = v2::Record::decode_from_slice(&wire).unwrap();
        let owned = v1::Record::decode_from_slice(&wire).unwrap();
        let view = v1::RecordView::decode_view(&wire).unwrap();
        let handle = v1::RecordOwnedView::decode(wire.to_vec().into()).unwrap();
        for roundtrip in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            handle.to_owned_message().encode_to_vec(),
        ] {
            assert_eq!(v2::Record::decode_from_slice(&roundtrip).unwrap(), expected);
        }
    }
}

#[test]
fn new_fields_enum_values_and_oneofs_survive_an_old_reader() {
    use evolution_fixture::{v1, v2};
    let mut newer = v2::Record::default().with_name("before");
    newer.mode = Some(v2::record::Mode::NEW);
    newer.extra = Some(7);
    newer.choice = Some(v2::record::Choice::Bytes(vec![1, 2, 3]));
    let wire = newer.encode_to_vec();
    let mut older = v1::Record::decode_from_slice(&wire).unwrap();
    older.name = Some("after".into());
    let restored = v2::Record::decode_from_slice(&older.encode_to_vec()).unwrap();
    assert_eq!(restored.name.as_deref(), Some("after"));
    assert_eq!(restored.mode, Some(v2::record::Mode::NEW));
    assert_eq!(restored.extra, Some(7));
    assert!(
        matches!(restored.choice, Some(v2::record::Choice::Bytes(bytes)) if bytes == [1, 2, 3])
    );
}

#[test]
fn editing_a_retained_future_enum_wins_for_a_new_reader() {
    use evolution_fixture::{v1, v2};
    let mut newer = v2::Record::default().with_name("fixture");
    newer.mode = Some(v2::record::Mode::NEW);
    newer.extra = Some(7);
    let wire = newer.encode_to_vec();
    let mut view = v1::RecordView::decode_view(&wire).unwrap();
    view.mode = Some(v1::record::Mode::READY);
    let restored = v2::Record::decode_from_slice(&view.encode_to_vec()).unwrap();
    assert_eq!(restored.mode, Some(v2::record::Mode::READY));
    assert_eq!(restored.extra, Some(7));
    let readers = [
        v1::Record::decode_from_slice(&wire).unwrap(),
        v1::RecordView::decode_view(&wire)
            .unwrap()
            .to_owned_message()
            .unwrap(),
        v1::RecordOwnedView::decode(wire.into())
            .unwrap()
            .to_owned_message(),
    ];
    for mut older in readers {
        older.mode = Some(v1::record::Mode::READY);
        let restored = v2::Record::decode_from_slice(&older.encode_to_vec()).unwrap();
        assert_eq!(restored.mode, Some(v2::record::Mode::READY));
        assert_eq!(restored.extra, Some(7));
    }
}

#[test]
fn editing_a_retained_future_oneof_wins_for_a_new_reader() {
    use evolution_fixture::{v1, v2};
    let mut newer = v2::Record::default().with_name("fixture");
    newer.choice = Some(v2::record::Choice::Bytes(vec![1, 2, 3]));
    newer.extra = Some(7);
    let wire = newer.encode_to_vec();
    let mut view = v1::RecordView::decode_view(&wire).unwrap();
    view.choice = Some(v1::record::ChoiceView::Text("edited"));
    let restored = v2::Record::decode_from_slice(&view.encode_to_vec()).unwrap();
    assert!(matches!(restored.choice, Some(v2::record::Choice::Text(text)) if text == "edited"));
    assert_eq!(restored.extra, Some(7));
    let readers = [
        v1::Record::decode_from_slice(&wire).unwrap(),
        v1::RecordView::decode_view(&wire)
            .unwrap()
            .to_owned_message()
            .unwrap(),
        v1::RecordOwnedView::decode(wire.into())
            .unwrap()
            .to_owned_message(),
    ];
    for mut older in readers {
        older.choice = Some(v1::record::Choice::Text("edited".into()));
        let restored = v2::Record::decode_from_slice(&older.encode_to_vec()).unwrap();
        assert!(
            matches!(restored.choice, Some(v2::record::Choice::Text(text)) if text == "edited")
        );
        assert_eq!(restored.extra, Some(7));
    }
}

#[test]
fn unrelated_edits_and_view_conversion_keep_each_received_winner() {
    use evolution_fixture::{v1, v2};
    for wire in [
        vec![0x10, 0, 0x10, 1, 0x1a, 1, b'a', 0x2a, 1, b'b'],
        vec![0x10, 1, 0x10, 0, 0x2a, 1, b'b', 0x1a, 1, b'a'],
    ] {
        let mut expected = v2::Record::decode_from_slice(&wire).unwrap();
        expected.name = Some("edited".into());
        let mut owned = v1::Record::decode_from_slice(&wire).unwrap();
        owned.name = Some("edited".into());
        let mut view = v1::RecordView::decode_view(&wire).unwrap();
        view.name = Some("edited");
        let mut handle = v1::RecordOwnedView::decode(wire.clone().into())
            .unwrap()
            .to_owned_message();
        handle.name = Some("edited".into());
        for bytes in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            view.to_owned_message().unwrap().encode_to_vec(),
            handle.encode_to_vec(),
        ] {
            assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
        }
    }
}

#[test]
fn changed_values_override_future_occurrences_after_all_conversion_paths() {
    use evolution_fixture::{v1, v2};
    for wire in [
        vec![0x10, 0, 0x10, 1, 0x1a, 1, b'a', 0x2a, 1, b'b'],
        vec![0x10, 1, 0x10, 0, 0x2a, 1, b'b', 0x1a, 1, b'a'],
    ] {
        let mut owned = v1::Record::decode_from_slice(&wire).unwrap();
        owned.mode = Some(v1::record::Mode::OTHER);
        owned.choice = Some(v1::record::Choice::Text("edited".into()));
        let mut view = v1::RecordView::decode_view(&wire).unwrap();
        view.mode = Some(v1::record::Mode::OTHER);
        view.choice = Some(v1::record::ChoiceView::Text("edited"));
        let mut handle = v1::RecordOwnedView::decode(wire.clone().into())
            .unwrap()
            .to_owned_message();
        handle.mode = Some(v1::record::Mode::OTHER);
        handle.choice = Some(v1::record::Choice::Text("edited".into()));
        for bytes in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            view.to_owned_message().unwrap().encode_to_vec(),
            handle.encode_to_vec(),
        ] {
            let decoded = v2::Record::decode_from_slice(&bytes).unwrap();
            assert_eq!(decoded.mode, Some(v2::record::Mode::OTHER));
            assert!(
                matches!(decoded.choice, Some(v2::record::Choice::Text(text)) if text == "edited")
            );
        }
    }
}

#[test]
fn setters_override_even_when_public_assignment_cannot_observe_a_change() {
    use evolution_fixture::{v1, v2};
    let wire = [0x10, 0, 0x10, 1, 0x1a, 1, b'a', 0x2a, 1, b'b'];
    let mut assigned = v1::Record::decode_from_slice(&wire).unwrap();
    assigned.mode = Some(v1::record::Mode::READY);
    assigned.choice = Some(v1::record::Choice::Text("a".into()));
    assert_eq!(
        v2::Record::decode_from_slice(&assigned.encode_to_vec()).unwrap(),
        v2::Record::decode_from_slice(&wire).unwrap()
    );
    let assigned = assigned
        .with_mode(v1::record::Mode::READY)
        .with_choice(v1::record::Choice::Text("a".into()));
    let decoded = v2::Record::decode_from_slice(&assigned.encode_to_vec()).unwrap();
    assert_eq!(decoded.mode, Some(v2::record::Mode::READY));
    assert!(matches!(decoded.choice, Some(v2::record::Choice::Text(text)) if text == "a"));
    let view = v1::RecordView::decode_view(&wire)
        .unwrap()
        .with_mode(v1::record::Mode::READY)
        .with_choice(v1::record::ChoiceView::Text("a"));
    for bytes in [
        view.encode_to_vec(),
        view.to_owned_message().unwrap().encode_to_vec(),
    ] {
        let decoded = v2::Record::decode_from_slice(&bytes).unwrap();
        assert_eq!(decoded.mode, Some(v2::record::Mode::READY));
        assert!(matches!(decoded.choice, Some(v2::record::Choice::Text(text)) if text == "a"));
    }
}

#[test]
fn clearing_the_known_projection_keeps_unrecognized_data() {
    use evolution_fixture::{v1, v2};
    let wire = [0x10, 1, 0x10, 0, 0x2a, 1, b'b', 0x1a, 1, b'a', 0x20, 7];
    let mut owned = v1::Record::decode_from_slice(&wire).unwrap();
    owned.mode = None;
    owned.choice = None;
    let decoded = v2::Record::decode_from_slice(&owned.encode_to_vec()).unwrap();
    assert_eq!(decoded.mode, Some(v2::record::Mode::NEW));
    assert_eq!(decoded.extra, Some(7));
    assert!(matches!(decoded.choice, Some(v2::record::Choice::Bytes(ref bytes)) if bytes == b"b"));
    let mut view = v1::RecordView::decode_view(&wire).unwrap();
    view.mode = None;
    view.choice = None;
    for bytes in [
        view.encode_to_vec(),
        view.to_owned_message().unwrap().encode_to_vec(),
    ] {
        assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), decoded);
    }
}

#[test]
fn nested_oneof_occurrences_do_not_merge_across_a_future_alternative() {
    use evolution_fixture::{v1, v2};
    // detail(left=1), future bytes="b", detail(right=2): the new schema resets
    // detail when switching away from bytes, while the old typed projection
    // alone merges left and right. Replay must preserve the new interpretation.
    let wire = [0x3a, 2, 8, 1, 0x2a, 1, b'b', 0x3a, 2, 16, 2];
    let expected = v2::Record::decode_from_slice(&wire).unwrap();
    let owned = v1::Record::decode_from_slice(&wire).unwrap();
    let view = v1::RecordView::decode_view(&wire).unwrap();
    let handle = v1::RecordOwnedView::decode(wire.to_vec().into()).unwrap();
    for bytes in [
        owned.encode_to_vec(),
        view.encode_to_vec(),
        view.to_owned_message().unwrap().encode_to_vec(),
        handle.to_owned_message().encode_to_vec(),
    ] {
        assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
    }
}

#[test]
fn view_conversion_canonicalization_is_not_a_user_edit() {
    use evolution_fixture::{v1, v2};
    // The child's unknown varint uses an overlong encoding. Owned conversion
    // canonicalizes it, but must not move detail after a later future choice.
    for wire in [
        vec![0x3a, 3, 0x20, 0x81, 0, 0x2a, 1, b'b'],
        vec![0x2a, 1, b'b', 0x3a, 3, 0x20, 0x81, 0],
    ] {
        let expected = v2::Record::decode_from_slice(&wire).unwrap();
        let owned = v1::Record::decode_from_slice(&wire).unwrap();
        let view = v1::RecordView::decode_view(&wire).unwrap();
        let handle = v1::RecordOwnedView::decode(wire.clone().into()).unwrap();
        for bytes in [
            owned.encode_to_vec(),
            view.encode_to_vec(),
            view.to_owned_message().unwrap().encode_to_vec(),
            handle.to_owned_message().encode_to_vec(),
        ] {
            assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
        }
    }
}

#[test]
fn edits_are_committed_before_a_later_wire_merge() {
    use evolution_fixture::{v1, v2};
    let first = [0x1a, 1, b'a', 0x2a, 1, b'b'];
    let later = [0x2a, 1, b'c', 0x1a, 1, b'd'];
    let mut owned = v1::Record::decode_from_slice(&first).unwrap();
    owned.choice = Some(v1::record::Choice::Text("edited".into()));
    owned.merge_from_slice(&later).unwrap();
    let decoded = v2::Record::decode_from_slice(&owned.encode_to_vec()).unwrap();
    assert!(matches!(decoded.choice, Some(v2::record::Choice::Text(text)) if text == "d"));
}

#[test]
fn occurrence_capture_crosses_buffer_chunks_and_rejects_truncation() {
    use evolution_fixture::{bytes::Buf, v1, v2};
    let wire = [0x2a, 1, b'b', 0x1a, 5, b'h', b'e', b'l', b'l', b'o'];
    let mut chunks = wire[..6].chain(&wire[6..]);
    let decoded = v1::Record::decode(&mut chunks).unwrap();
    let cloned = decoded.clone();
    drop(decoded);
    assert_eq!(
        v2::Record::decode_from_slice(&cloned.encode_to_vec()).unwrap(),
        v2::Record::decode_from_slice(&wire).unwrap()
    );
    assert!(v1::Record::decode_from_slice(&wire[..8]).is_err());
}

#[test]
fn cloned_owned_view_keeps_borrowed_occurrences_after_original_drop() {
    use evolution_fixture::{v1, v2};
    let wire = [0x2a, 1, b'b', 0x3a, 2, 16, 2];
    let expected = v2::Record::decode_from_slice(&wire).unwrap();
    let handle = v1::RecordOwnedView::decode(wire.to_vec().into()).unwrap();
    let cloned = handle.clone();
    drop(handle);
    for bytes in [
        cloned.view().encode_to_vec(),
        cloned.to_owned_message().encode_to_vec(),
    ] {
        assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
    }
}

#[test]
fn occurrence_metadata_obeys_the_existing_decode_memory_budget() {
    use evolution_fixture::{DecodeError, DecodeOptions, v1};
    let bounded = DecodeOptions::new().with_element_memory_limit(512);
    let mut wire = Vec::new();
    for _ in 0..100 {
        wire.extend_from_slice(&[0x20, 7]);
    }
    assert!(matches!(
        bounded.decode_from_slice::<v1::Record>(&wire),
        Err(DecodeError::ElementMemoryLimitExceeded)
    ));
    assert!(matches!(
        bounded.decode_view::<v1::RecordView<'_>>(&wire),
        Err(DecodeError::ElementMemoryLimitExceeded)
    ));
    assert!(
        DecodeOptions::new()
            .with_element_memory_limit(0)
            .decode_from_slice::<v1::Record>(&[0x10, 0])
            .is_ok()
    );
    let allowed = DecodeOptions::new().with_element_memory_limit(32 * 1024);
    let owned = allowed.decode_from_slice::<v1::Record>(&wire).unwrap();
    let view = allowed.decode_view::<v1::RecordView<'_>>(&wire).unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn consecutive_future_fields_reuse_the_unchanged_oneof_projection() {
    use evolution_fixture::{DecodeContext, DecodeOptions, v1};
    let mut message = v1::Record::default();
    message.choice = Some(v1::record::Choice::Text("x".repeat(16 * 1024)));
    let mut wire = message.encode_to_vec();
    for _ in 0..20 {
        wire.extend_from_slice(&[0x20, 7]);
    }
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options.decode_from_slice::<v1::Record>(&wire).unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    let view = options.decode_view::<v1::RecordView<'_>>(&wire).unwrap();
    assert_eq!(view.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
    let mut group_wire = wire.clone();
    group_wire.extend_from_slice(&[0x9c, 0x06]); // EndGroup, field 99.
    let unknown_limit = core::cell::Cell::new(1024 * 1024);
    let memory_limit = core::cell::Cell::new(128 * 1024);
    let ctx = DecodeContext::new(100, &unknown_limit).with_element_memory(&memory_limit);
    let mut group = v1::Record::default();
    group
        .merge_group(&mut group_wire.as_slice(), ctx, 99)
        .unwrap();
    assert_eq!(group.encode_to_vec(), wire);
    // An empty group does not modify or allocate anything, even when merging
    // into an owner whose retained projection was paid by an earlier decode.
    let memory_limit = core::cell::Cell::new(0);
    let ctx = DecodeContext::new(100, &unknown_limit).with_element_memory(&memory_limit);
    group
        .merge_group(&mut [0x9c, 0x06].as_slice(), ctx, 99)
        .unwrap();
    assert_eq!(group.encode_to_vec(), wire);
}

fn check_repeated_enum_order(wire: &[u8]) {
    use evolution_fixture::{v1, v2};
    let owned = v1::RepeatedRecord::decode_from_slice(wire).unwrap();
    let view = v1::RepeatedRecordView::decode_view(wire).unwrap();
    let values: Vec<_> = [
        owned.encode_to_vec(),
        view.encode_to_vec(),
        view.to_owned_message().unwrap().encode_to_vec(),
    ]
    .into_iter()
    .map(|bytes| v2::RepeatedRecord::decode_from_slice(&bytes).unwrap().modes)
    .collect();
    let expected = v2::RepeatedRecord::decode_from_slice(wire).unwrap().modes;
    assert_eq!(values, vec![expected; 3]);
}

#[test]
fn repeated_future_enum_keeps_unpacked_positions() {
    check_repeated_enum_order(&[0x08, 0, 0x08, 1, 0x08, 2]);
}

#[test]
fn repeated_future_enum_keeps_packed_positions() {
    check_repeated_enum_order(&[0x0a, 3, 0, 1, 2]);
}

#[test]
fn repeated_future_enum_keeps_mixed_records_and_clones() {
    use evolution_fixture::{v1, v2};
    let wire = [0x0a, 3, 0, 1, 2, 0x08, 1, 0x0a, 2, 2, 0];
    check_repeated_enum_order(&wire);
    let owned = v1::RepeatedRecord::decode_from_slice(&wire).unwrap().clone();
    let view = v1::RepeatedRecordView::decode_view(&wire).unwrap().clone();
    let expected = v2::RepeatedRecord::decode_from_slice(&wire).unwrap().modes;
    for encoded in [owned.encode_to_vec(), view.encode_to_vec()] {
        assert_eq!(v2::RepeatedRecord::decode_from_slice(&encoded).unwrap().modes, expected);
    }
}

#[test]
fn repeated_future_enum_preserves_declared_packed_field() {
    use evolution_fixture::{v1, v2};
    let wire = [0x12, 3, 0, 1, 2, 0x10, 1, 0x12, 2, 2, 0];
    let owned = v1::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let view = v1::RepeatedRecordView::decode_view(&wire).unwrap();
    let expected = v2::RepeatedRecord::decode_from_slice(&wire).unwrap().packed_modes;
    for encoded in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
        assert_eq!(v2::RepeatedRecord::decode_from_slice(&encoded).unwrap().packed_modes, expected);
    }
}

#[test]
fn repeated_future_enum_public_edits_and_explicit_replacement() {
    use evolution_fixture::{v1, v2};
    let wire = [0x0a, 3, 0, 1, 2];
    let mut owned = v1::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let mut view = v1::RepeatedRecordView::decode_view(&wire).unwrap();
    owned.modes.reverse();
    view.modes = owned.modes.clone().into();
    for encoded in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
        assert_eq!(v2::RepeatedRecord::decode_from_slice(&encoded).unwrap().modes, vec![v2::record::Mode::NEW, v2::record::Mode::OTHER, v2::record::Mode::READY]);
    }
    let owned = v1::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let values = owned.modes.clone();
    let replaced = owned.with_modes(values);
    assert_eq!(v2::RepeatedRecord::decode_from_slice(&replaced.encode_to_vec()).unwrap().modes, vec![v2::record::Mode::NEW, v2::record::Mode::READY, v2::record::Mode::OTHER]);
}

#[test]
fn repeated_future_enum_reconciles_edits_between_decode_calls() {
    use evolution_fixture::{DecodeContext, v1, v2};
    let wire = [0x0a, 3, 0, 1, 2];
    let mut owned = v1::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let mut view = v1::RepeatedRecordView::decode_view(&wire).unwrap();
    owned.modes.reverse();
    view.modes = owned.modes.clone().into();
    let appended = [0x0a, 1, 0];
    owned.merge_from_slice(&appended).unwrap();
    let unknown_limit = core::cell::Cell::new(1024 * 1024);
    view.merge_into_view(&appended, DecodeContext::new(100, &unknown_limit)).unwrap();
    for encoded in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
        assert_eq!(v2::RepeatedRecord::decode_from_slice(&encoded).unwrap().modes, vec![v2::record::Mode::NEW, v2::record::Mode::OTHER, v2::record::Mode::READY, v2::record::Mode::READY]);
    }
}

#[test]
fn repeated_future_enum_packed_boundaries_and_memory_budget() {
    use evolution_fixture::{DecodeError, DecodeOptions, v1};
    for wire in [&[0x0a, 1, 0x80][..], &[0x0a, 3, 0][..]] {
        assert!(v1::RepeatedRecord::decode_from_slice(wire).is_err());
        assert!(v1::RepeatedRecordView::decode_view(wire).is_err());
    }
    let wire = [0x0a, 3, 0, 1, 2];
    let zero = DecodeOptions::new().with_element_memory_limit(0);
    assert!(matches!(zero.decode_from_slice::<v1::RepeatedRecord>(&wire), Err(DecodeError::ElementMemoryLimitExceeded)));
    assert!(matches!(zero.decode_view::<v1::RepeatedRecordView<'_>>(&wire), Err(DecodeError::ElementMemoryLimitExceeded)));
    let allowed = DecodeOptions::new().with_element_memory_limit(4096);
    assert!(allowed.decode_from_slice::<v1::RepeatedRecord>(&wire).is_ok());
    assert!(allowed.decode_view::<v1::RepeatedRecordView<'_>>(&wire).is_ok());
}

#[test]
fn repeated_future_enum_long_lists_have_linear_decode_budget() {
    use evolution_fixture::{DecodeOptions, v1, v2};
    for field in [1, 2] {
        for packed in [false, true] {
            let mut wire = vec![field << 3, 1];
            if packed {
                wire.extend_from_slice(&[(field << 3) | 2, 0x80, 4]);
                wire.extend(std::iter::repeat_n(0, 512));
            } else {
                for _ in 0..512 { wire.extend_from_slice(&[field << 3, 0]); }
            }
            let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
            let owned = options.decode_from_slice::<v1::RepeatedRecord>(&wire).unwrap();
            let view = options.decode_view::<v1::RepeatedRecordView<'_>>(&wire).unwrap();
            let expected = v2::RepeatedRecord::decode_from_slice(&wire).unwrap();
            for encoded in [owned.encode_to_vec(), view.encode_to_vec(), view.to_owned_message().unwrap().encode_to_vec()] {
                assert_eq!(v2::RepeatedRecord::decode_from_slice(&encoded).unwrap(), expected);
            }
        }
    }
}

#[test]
fn raw_identifier_setter_replaces_a_same_value_explicitly() {
    use evolution_fixture::{v1, v2};
    let wire = [0x20, 0, 0x20, 1];
    let owned = v1::EnumRecord::decode_from_slice(&wire).unwrap().with_type(v1::record::Mode::READY);
    let restored = v2::EnumRecord::decode_from_slice(&owned.encode_to_vec()).unwrap();
    assert_eq!(restored.r#type, Some(v2::record::Mode::READY));
}

#[test]
fn decode_batches_reconcile_edits_before_unrelated_fields() {
    use evolution_fixture::{DecodeContext, v1, v2};
    let received = [0x1a, 1, b'a', 0x2a, 1, b'b'];
    // The first field changes an unrelated string; the last is a new future
    // alternative that must win over the edit made between decode calls.
    let appended = [0x0a, 1, b'n', 0x20, 7, 0x2a, 1, b'c'];
    let mut owned = v1::Record::decode_from_slice(&received).unwrap();
    owned.choice = Some(v1::record::Choice::Text("edited".into()));
    owned.merge_from_slice(&appended).unwrap();
    let restored = v2::Record::decode_from_slice(&owned.encode_to_vec()).unwrap();
    assert_eq!(restored.choice, Some(v2::record::Choice::Bytes(vec![b'c'])));
    let mut view = v1::RecordView::decode_view(&received).unwrap();
    view.choice = Some(v1::record::ChoiceView::Text("edited"));
    let unknown_limit = core::cell::Cell::new(1024 * 1024);
    view.merge_into_view(&appended, DecodeContext::new(100, &unknown_limit))
        .unwrap();
    let restored = v2::Record::decode_from_slice(&view.encode_to_vec()).unwrap();
    assert_eq!(restored.choice, Some(v2::record::Choice::Bytes(vec![b'c'])));
}

#[test]
fn removing_raw_unknowns_discards_the_journal_without_changing_empty_equality() {
    use evolution_fixture::v1;
    use std::hash::{Hash, Hasher};
    let mut record = v1::Record::decode_from_slice(&[0x10, 1]).unwrap();
    record.__buffa_unknown_fields.retain(|_| false);
    let empty = v1::Record::default();
    assert_eq!(record, empty);
    let mut left = std::collections::hash_map::DefaultHasher::new();
    let mut right = std::collections::hash_map::DefaultHasher::new();
    record.__buffa_unknown_fields.hash(&mut left);
    empty.__buffa_unknown_fields.hash(&mut right);
    assert_eq!(left.finish(), right.finish());
}

#[test]
fn nested_future_records_reuse_prepared_output_across_encoding_passes() {
    use evolution_fixture::{SizeCache, v1, v2};
    let mut wire = vec![0x10, 1, 0x10, 0];
    for _ in 0..24 {
        let mut parent = vec![0x10, 1, 0x42];
        evolution_fixture::encoding::encode_varint(wire.len() as u64, &mut parent);
        parent.extend_from_slice(&wire);
        parent.extend_from_slice(&[0x10, 0]);
        wire = parent;
    }
    let expected = v2::Record::decode_from_slice(&wire).unwrap();
    let owned = v1::Record::decode_from_slice(&wire).unwrap();
    let view = v1::RecordView::decode_view(&wire).unwrap();
    for bytes in [
        owned.encode_to_vec(),
        view.encode_to_vec(),
        view.to_owned_message().unwrap().encode_to_vec(),
    ] {
        assert_eq!(v2::Record::decode_from_slice(&bytes).unwrap(), expected);
    }
    // A preceding/following normal codec must consume exactly its own slots;
    // clearing a reused traversal cache must also discard prepared output.
    let plain = v1::Record::default().with_name("plain");
    let mut cache = SizeCache::new();
    for _ in 0..2 {
        let size = plain.compute_size(&mut cache)
            + owned.compute_size(&mut cache)
            + plain.compute_size(&mut cache);
        let mut encoded = Vec::new();
        plain.write_to(&mut cache, &mut encoded);
        owned.write_to(&mut cache, &mut encoded);
        plain.write_to(&mut cache, &mut encoded);
        assert_eq!(encoded.len(), size as usize);
        cache.clear();
    }
}

#[test]
fn repeated_groups_keep_equality_and_hash_across_merge_batches() {
    use evolution_fixture::{v1, DecodeContext};
    use std::hash::{Hash, Hasher};
    let first = [8, 1, 8, 0, 16, 0];
    let later = [24, 0, 8, 0, 16, 0];
    let mut wire = first.to_vec();
    wire.extend_from_slice(&later);
    let batch = v1::RepeatedRecord::decode_from_slice(&wire).unwrap();
    let mut direct = v1::RepeatedRecord::decode_from_slice(&first).unwrap();
    for field in later.chunks_exact(2) { direct.merge_from_slice(field).unwrap(); }
    assert_eq!(batch.encode_to_vec(), direct.encode_to_vec());
    assert_eq!(batch, direct);
    let hash = |value: &v1::RepeatedRecord| {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        value.__buffa_unknown_fields.hash(&mut hash);
        hash.finish()
    };
    assert_eq!(hash(&batch), hash(&direct));
    let unknown = core::cell::Cell::new(usize::MAX);
    let ctx = DecodeContext::new(100, &unknown);
    let view_batch = v1::RepeatedRecordView::decode_view(&wire).unwrap();
    let mut view_direct = v1::RepeatedRecordView::decode_view(&first).unwrap();
    for field in later.chunks_exact(2) { view_direct.merge_into_view(field, ctx).unwrap(); }
    assert_eq!(view_batch.encode_to_vec(), view_direct.encode_to_vec());
    assert_eq!(view_batch.to_owned_message().unwrap(), view_direct.to_owned_message().unwrap());
}

mod semantic_projection_differential {
use evolution_fixture::{Message as _, MessageView as _, v1, v2, semantic_projection::{Arena, Field, Kind, ValueRef, Visitor}};
const ROOT: &[Field] = &[
    Field { number: 1, kind: Kind::Bytes, repeated: false, oneof: 0 },
    Field { number: 6, kind: Kind::Message(1), repeated: false, oneof: 0 },
    Field { number: 3, kind: Kind::Bytes, repeated: false, oneof: 1 },
    Field { number: 7, kind: Kind::Message(1), repeated: false, oneof: 1 },
];
const CHILD: &[Field] = &[
    Field { number: 1, kind: Kind::Uint32, repeated: false, oneof: 0 },
    Field { number: 2, kind: Kind::Uint32, repeated: false, oneof: 0 },
    Field { number: 3, kind: Kind::Uint32, repeated: true, oneof: 0 },
    Field { number: 32, kind: Kind::Uint32, repeated: true, oneof: 0 },
];
const SCHEMA: &[&[Field]] = &[ROOT, CHILD];

struct ChildVisitor<'a>(&'a v1::record::Child);
impl Visitor for ChildVisitor<'_> {
    fn field(&self, number: u32, index: usize) -> Option<ValueRef<'_>> {
        let value = match number {
            1 if index == 0 => self.0.left,
            2 if index == 0 => self.0.right,
            3 => self.0.values.get(index).copied(),
            32 => self.0.wide_values.get(index).copied(),
            _ => None,
        }?;
        Some(ValueRef::Varint(u64::from(value)))
    }
    fn unknown(&self, index: usize) -> Option<(u32, ValueRef<'_>)> {
        use evolution_fixture::UnknownFieldData;
        let field = self.0.__buffa_unknown_fields.iter().nth(index)?;
        let value = match &field.data {
            UnknownFieldData::Varint(value) => ValueRef::Varint(*value),
            UnknownFieldData::Fixed32(value) => ValueRef::Fixed32(*value),
            UnknownFieldData::Fixed64(value) => ValueRef::Fixed64(*value),
            UnknownFieldData::LengthDelimited(value) => ValueRef::Bytes(value),
            UnknownFieldData::Group(_) => ValueRef::Unsupported,
        };
        Some((field.number, value))
    }
}
struct RecordVisitor<'a> {
    record: &'a v1::Record,
    detail: Option<ChildVisitor<'a>>,
    child: Option<ChildVisitor<'a>>,
}
impl<'a> RecordVisitor<'a> {
    fn new(record: &'a v1::Record) -> Self {
        let detail = match record.choice.as_ref() { Some(v1::record::Choice::Detail(value)) => Some(ChildVisitor(value.as_ref())), _ => None };
        Self { record, detail, child: record.child.as_option().map(ChildVisitor) }
    }
}
impl Visitor for RecordVisitor<'_> {
    fn supported(&self) -> bool { self.record.mode.is_none() && self.record.next.is_unset() }
    fn field(&self, number: u32, index: usize) -> Option<ValueRef<'_>> {
        use v1::record::Choice;
        if index != 0 { return None; }
        match number {
            1 => self.record.name.as_deref().map(|value| ValueRef::Bytes(value.as_bytes())),
            6 => self.child.as_ref().map(|value| ValueRef::Child(value)),
            3 => match self.record.choice.as_ref()? { Choice::Text(value) => Some(ValueRef::Bytes(value.as_bytes())), _ => None },
            7 => self.detail.as_ref().map(|value| ValueRef::Child(value)),
            _ => None,
        }
    }
}
#[test]
fn completed_occurrences_match_typed_codec_and_future_reader() {
    let events: &[&[u8]] = &[
        &[0x3a, 2, 8, 1], &[0x3a, 2, 16, 2], &[0x1a, 1, b'a'],
        &[0x3a, 0], &[0x3a, 2, 8, 0], &[0x3a, 6, 0x18, 1, 0x1a, 2, 2, 3],
        &[0x3a, 3, 0x20, 0x81, 0], &[0x2a, 1, b'b'],
    ];
    for a in events { for b in events { for c in events {
        let unknown = std::cell::Cell::new(usize::MAX);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let mut projection = Arena::new(SCHEMA);
        let mut known = Vec::new();
        let wire = [*a, *b, *c].concat();
        for record in [*a, *b, *c] {
            if record[0] != 0x2a {
                projection.accept(record, ctx).unwrap();
                known.extend_from_slice(record);
            }
        }
        let typed = v1::Record::decode_from_slice(&known).unwrap();
        assert!(projection.matches(&RecordVisitor::new(&typed)), "semantic comparison {wire:?}");
        let direct = Arena::from_visitor(SCHEMA, &RecordVisitor::new(&typed), ctx).unwrap();
        assert_eq!(projection.encode(), direct.encode(), "direct visitor baseline {wire:?}");
        assert!(projection.matches(&RecordVisitor::new(&typed.clone())), "clone {wire:?}");
        let view = v1::RecordView::decode_view(&known).unwrap();
        assert!(projection.matches(&RecordVisitor::new(&view.to_owned_message().unwrap())), "view conversion {wire:?}");
        assert_eq!(projection.encode(), typed.encode_to_vec(), "wire {wire:?}");
        // The semantic baseline must never replace the ordered replay tape.
        let original = v1::Record::decode_from_slice(&wire).unwrap();
        assert_eq!(v2::Record::decode_from_slice(&original.encode_to_vec()).unwrap(), v2::Record::decode_from_slice(&wire).unwrap());
    } } }
}
#[test]
fn failed_occurrence_does_not_bless_the_typed_receivers_partial_edit() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
    let first = [0x3a, 2, 8, 1, 0x2a, 1, b'b'];
    let completed = [0x3a, 2, 16, 2];
    let failed = [0x3a, 4, 8, 3, 0xc0, 0x3e];
    let mut projection = Arena::new(SCHEMA);
    projection.accept(&first[..4], ctx).unwrap();
    projection.accept(&completed, ctx).unwrap();
    let baseline = projection.encode();
    assert!(projection.accept(&failed, ctx).is_err());
    assert_eq!(projection.encode(), baseline);
    let mut current = v1::Record::decode_from_slice(&first).unwrap();
    assert!(current.merge_from_slice(&[completed.as_slice(), failed.as_slice()].concat()).is_err());
    assert!(!projection.matches(&RecordVisitor::new(&current)), "partial mutation is not in the completed baseline");
    let old_baseline = v1::Record::decode_from_slice(&baseline).unwrap();
    assert_ne!(current.choice, old_baseline.choice);
    current.merge_from_slice(&[0x3a, 0]).unwrap();
    projection.accept(&[0x3a, 0], ctx).unwrap();
    assert_eq!(projection.encode(), baseline);
    let evolved = v2::Record::decode_from_slice(&current.encode_to_vec()).unwrap();
    let expected = v2::Record::decode_from_slice(&[0x2a, 1, b'b', 0x3a, 4, 8, 3, 16, 2]).unwrap();
    assert_eq!(evolved.choice, expected.choice);
}
// The prototype receiver below handles the fixture's text/detail oneof and
// future field5 directly; unsupported shapes migrate to the original codec.
// Production codecs are untouched.
#[derive(Clone)]
enum ReplayEvent { Prefix(std::sync::Arc<Arena>), Known(std::ops::Range<usize>), Future(Vec<u8>) }
#[derive(Clone)]
struct ProjectedRecord { current: v1::Record, baseline: Arena, events: Vec<ReplayEvent>, active: bool, eager: bool }
impl ProjectedRecord {
    fn new() -> Self { Self { current: v1::Record::default(), baseline: Arena::new(SCHEMA), events: Vec::new(), active: false, eager: false } }
    fn supports_current(&self) -> bool {
        RecordVisitor::new(&self.current).supported()
            && self.current.name.is_none() && self.current.child.is_unset()
            && self.current.__buffa_unknown_fields.is_empty()
            && match self.current.choice.as_ref() {
                Some(v1::record::Choice::Detail(child)) => !child.__buffa_unknown_fields.iter().any(|field| matches!(field.data, evolution_fixture::UnknownFieldData::Group(_))),
                _ => true,
            }
    }
    fn migrate(&mut self, ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        let bound = if self.active {
            self.events.iter().fold(0usize, |total, event| total.saturating_add(match event {
                ReplayEvent::Prefix(prefix) => prefix.encoding_bound(),
                ReplayEvent::Known(raw) => raw.len(),
                ReplayEvent::Future(raw) => raw.len(),
            }))
        } else { self.baseline.encoding_bound() };
        ctx.register_element_memory(bound)?;
        let mut wire = Vec::with_capacity(bound);
        if self.active {
            for event in &self.events {
                match event {
                    ReplayEvent::Prefix(prefix) => prefix.encode_into(&mut wire),
                    ReplayEvent::Known(raw) => wire.extend_from_slice(self.baseline.record(raw)),
                    ReplayEvent::Future(raw) => wire.extend_from_slice(raw),
                }
            }
        } else { self.baseline.encode_into(&mut wire); }
        // Accepted unknowns do not debit the caller again. Replay still pays
        // unknown metadata from its local element balance, including on error.
        let remaining = ctx.remaining_element_memory().unwrap_or(usize::MAX);
        let budget = std::cell::Cell::new(remaining);
        let replay_ctx = evolution_fixture::DecodeContext::new(u32::MAX, &budget).with_element_memory(&budget);
        let mut prepared = v1::Record::default();
        let result = prepared.merge(&mut wire.as_slice(), replay_ctx);
        ctx.register_element_memory(remaining.saturating_sub(budget.get()))?;
        result?;
        ctx.register_element_memory(self.current.__buffa_unknown_fields.len().saturating_mul(size_of::<evolution_fixture::UnknownField>()))?;
        // Publish only after fallible preparation. Move public fields rather
        // than cloning them or blessing a partial receiver as the baseline.
        prepared.name = std::mem::take(&mut self.current.name);
        prepared.mode = std::mem::take(&mut self.current.mode);
        prepared.choice = std::mem::take(&mut self.current.choice);
        prepared.child = std::mem::take(&mut self.current.child);
        prepared.next = std::mem::take(&mut self.current.next);
        for field in std::mem::take(&mut self.current.__buffa_unknown_fields) {
            prepared.__buffa_unknown_fields.push(field);
        }
        self.current = prepared;
        self.events.clear();
        self.baseline = Arena::new(SCHEMA);
        self.active = false;
        self.eager = true;
        Ok(())
    }
    fn reconcile(&mut self, ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        if self.eager { return Ok(()); }
        if !self.supports_current() { return self.migrate(ctx); }
        if !self.active { return Ok(()); }
        if self.baseline.matches(&RecordVisitor::new(&self.current)) { return Ok(()); }
        if let Some(edit) = self.baseline.scalar_edit(&RecordVisitor::new(&self.current)) {
            let prefix = if self.active {
                ctx.register_element_memory(size_of::<ReplayEvent>().saturating_add(size_of::<Arena>()).saturating_add(2 * size_of::<usize>()))?;
                let mut prefix = self.baseline.clone_with_context(ctx)?;
                prefix.apply_scalar(edit.clone());
                prefix.compact_projection();
                Some(prefix)
            } else { None };
            self.events.retain(|event| matches!(event, ReplayEvent::Future(_)));
            self.baseline.apply_scalar(edit);
            self.baseline.compact_projection();
            if let Some(prefix) = prefix { self.events.push(ReplayEvent::Prefix(std::sync::Arc::new(prefix))); }
            return Ok(());
        }
        let current = Arena::from_visitor(SCHEMA, &RecordVisitor::new(&self.current), ctx)?;
        if self.active {
            ctx.register_element_memory(size_of::<ReplayEvent>().saturating_add(size_of::<Arena>()).saturating_add(2 * size_of::<usize>()))?;
            let prefix = current.clone_with_context(ctx)?;
            self.events.retain(|event| matches!(event, ReplayEvent::Future(_)));
            self.events.push(ReplayEvent::Prefix(std::sync::Arc::new(prefix)));
        }
        self.baseline = current;
        Ok(())
    }
    fn merge_record(&mut self, record: &[u8], ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        self.reconcile(ctx)?;
        self.merge_record_inner(record, ctx)
    }
    fn merge_records(&mut self, records: &[&[u8]], ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        self.reconcile(ctx)?;
        for record in records { self.merge_record_inner(record, ctx)?; }
        Ok(())
    }
    #[cfg(feature = "semantic-bench")]
    fn merge_wire(&mut self, wire: &[u8], ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        self.reconcile(ctx)?;
        let mut input = wire;
        while !input.is_empty() {
            let before = input;
            let tag = evolution_fixture::encoding::Tag::decode(&mut input)?;
            assert_eq!(tag.wire_type(), evolution_fixture::encoding::WireType::LengthDelimited);
            evolution_fixture::types::borrow_bytes(&mut input)?;
            self.merge_record_inner(&before[..before.len() - input.len()], ctx)?;
        }
        Ok(())
    }
    fn merge_record_inner(&mut self, record: &[u8], ctx: evolution_fixture::DecodeContext<'_>) -> Result<(), evolution_fixture::DecodeError> {
        if self.eager { return self.current.merge(&mut &record[..], ctx); }
        let tag = evolution_fixture::encoding::Tag::decode(&mut &record[..])?;
        if !matches!(tag.field_number(), 3 | 5 | 7)
            || (tag.field_number() == 5 && record.first() != Some(&0x2a))
            || tag.wire_type() != evolution_fixture::encoding::WireType::LengthDelimited {
            self.migrate(ctx)?;
            return self.current.merge(&mut &record[..], ctx);
        }
        if record.first() == Some(&0x2a) {
            let mut bytes = &record[1..];
            evolution_fixture::types::borrow_bytes(&mut bytes)?;
            assert!(bytes.is_empty(), "one complete fixture occurrence");
            ctx.register_element_memory(size_of::<ReplayEvent>().saturating_add(record.len()))?;
            ctx.register_unknown_field()?;
            if !self.active {
                let baseline = Arena::from_visitor(SCHEMA, &RecordVisitor::new(&self.current), ctx)?;
                if !baseline.is_empty() {
                    ctx.register_element_memory(size_of::<ReplayEvent>().saturating_add(size_of::<Arena>()).saturating_add(2 * size_of::<usize>()))?;
                    let prefix = baseline.clone_with_context(ctx)?;
                    self.events.push(ReplayEvent::Prefix(std::sync::Arc::new(prefix)));
                }
                self.baseline = baseline;
                self.active = true;
            }
            self.events.push(ReplayEvent::Future(record.to_vec()));
            return Ok(());
        }
        if !self.active { return self.current.merge(&mut &record[..], ctx); }
        let staged = self.baseline.stage(record, ctx);
        if matches!(&staged, Err(evolution_fixture::DecodeError::InvalidWireType(3))) {
            drop(staged);
            self.migrate(ctx)?;
            return self.current.merge(&mut &record[..], ctx);
        }
        let staged = match staged {
            Ok(staged) => staged,
            Err(evolution_fixture::DecodeError::ElementMemoryLimitExceeded) => return Err(evolution_fixture::DecodeError::ElementMemoryLimitExceeded),
            Err(error) => {
                // Validation must not skip the typed decoder's valid partial
                // updates. This closure's parser errors mirror its codec.
                self.current.merge(&mut &record[..], ctx)?;
                return Err(error);
            }
        };
        if self.active { ctx.register_element_memory(size_of::<ReplayEvent>())?; }
        self.current.merge(&mut &record[..], ctx)?;
        let raw = staged.commit();
        if self.active { self.events.push(ReplayEvent::Known(raw)); }
        Ok(())
    }
    fn raw_unknowns_mut(&mut self, ctx: evolution_fixture::DecodeContext<'_>) -> Result<&mut evolution_fixture::UnknownFields, evolution_fixture::DecodeError> {
        if !self.eager { self.migrate(ctx)?; }
        Ok(&mut self.current.__buffa_unknown_fields)
    }
    fn encode(&self) -> Vec<u8> {
        if !self.eager && !self.supports_current() {
            let mut migrated = self.clone();
            let unlimited = std::cell::Cell::new(usize::MAX);
            migrated.migrate(evolution_fixture::DecodeContext::new(u32::MAX, &unlimited)).expect("unbudgeted fixture migration");
            return migrated.current.encode_to_vec();
        }
        if self.eager { return self.current.encode_to_vec(); }
        if !self.active { return self.current.encode_to_vec(); }
        let unchanged = self.baseline.matches(&RecordVisitor::new(&self.current));
        let mut bytes = Vec::new();
        for event in &self.events {
            match event {
                ReplayEvent::Prefix(prefix) if unchanged => bytes.extend(prefix.encode()),
                ReplayEvent::Known(raw) if unchanged => bytes.extend_from_slice(self.baseline.record(raw)),
                ReplayEvent::Future(raw) => bytes.extend_from_slice(raw),
                _ => {},
            }
        }
        if !unchanged { bytes.extend(self.current.encode_to_vec()); }
        bytes
    }
}
#[test]
fn prototype_known_only_decode_needs_no_semantic_bookkeeping_budget() {
    let unknown = std::cell::Cell::new(100);
    let budget = std::cell::Cell::new(0);
    let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
    let mut prototype = ProjectedRecord::new();
    prototype.merge_records(&[&[0x3a, 2, 8, 1], &[0x3a, 2, 16, 2]], ctx.with_element_memory(&budget)).unwrap();
    assert!(prototype.baseline.is_empty());
    assert!(!prototype.active && !prototype.eager);
    assert!(prototype.merge_record(&[0x3a, 4, 8, 3, 0xc0, 0x3e], ctx.with_element_memory(&budget)).is_err());
    assert!(prototype.merge_record(&[0x2a, 1, b'b'], ctx.with_element_memory(&budget)).is_err());
    assert!(prototype.baseline.is_empty());
    assert!(!prototype.active);
    prototype.merge_record(&[0x2a, 1, b'b'], ctx).unwrap();
    prototype.merge_record(&[0x3a, 0], ctx).unwrap();
    let mut original = v1::Record::default();
    original.merge_from_slice(&[0x3a, 4, 8, 3, 16, 2]).unwrap();
    original.merge_from_slice(&[0x2a, 1, b'b', 0x3a, 0]).unwrap();
    assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&original.encode_to_vec()).unwrap());
}
#[test]
fn prototype_migration_delegates_received_unsupported_fields_to_original_codec() {
    for unsupported in [&[0x10, 0][..], &[0x10, 1], &[0x42, 0], &[0xaa, 0, 1, b'z'], &[0x3a, 4, 0x2b, 8, 1, 0x2c]] {
        let unknown = std::cell::Cell::new(100);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let records: &[&[u8]] = &[&[0x3a, 2, 8, 1], &[0x2a, 1, b'b'], &[0x3a, 2, 16, 2]];
        let mut prototype = ProjectedRecord::new();
        let mut original = v1::Record::default();
        for record in records {
            prototype.merge_record(record, ctx).unwrap();
            original.merge_from_slice(record).unwrap();
        }
        prototype.merge_record(unsupported, ctx).unwrap();
        original.merge_from_slice(unsupported).unwrap();
        assert!(prototype.eager);
        prototype.merge_record(&[0x3a, 0], ctx).unwrap();
        original.merge_from_slice(&[0x3a, 0]).unwrap();
        assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&original.encode_to_vec()).unwrap());
    }
}
#[test]
fn prototype_migration_preserves_unsupported_public_edits() {
    for next in [false, true] {
        let unknown = std::cell::Cell::new(100);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let wire = [0x3a, 2, 8, 1, 0x2a, 1, b'b'];
        let mut prototype = ProjectedRecord::new();
        prototype.merge_records(&[&wire[..4], &wire[4..]], ctx).unwrap();
        let mut original = v1::Record::decode_from_slice(&wire).unwrap();
        if next {
            prototype.current.next = Some(v1::Record::default()).into();
            original.next = Some(v1::Record::default()).into();
        } else {
            prototype.current.mode = Some(v1::record::Mode::READY);
            original.mode = Some(v1::record::Mode::READY);
        }
        prototype.merge_record(&[0x3a, 0], ctx).unwrap();
        original.merge_from_slice(&[0x3a, 0]).unwrap();
        assert!(prototype.eager);
        assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&original.encode_to_vec()).unwrap());
    }
}
#[test]
fn prototype_migration_failures_preserve_partial_receiver_and_allow_retry() {
    let mut rejected = 0;
    let mut admitted = 0;
    for quota in 0..1024 {
        let unknown = std::cell::Cell::new(100);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let records: &[&[u8]] = &[&[0x3a, 2, 8, 1], &[0x2a, 1, b'b'], &[0x3a, 2, 16, 2]];
        let mut prototype = ProjectedRecord::new();
        let mut original = v1::Record::default();
        for record in records {
            prototype.merge_record(record, ctx).unwrap();
            original.merge_from_slice(record).unwrap();
        }
        let malformed = [0x3a, 4, 8, 3, 0xc0, 0x3e];
        assert!(prototype.merge_record(&malformed, ctx).is_err());
        assert!(original.merge_from_slice(&malformed).is_err());
        let before = prototype.encode();
        // Replayed unknowns use the element balance, not this allowance.
        unknown.set(0);
        let budget = std::cell::Cell::new(quota);
        match prototype.migrate(ctx.with_element_memory(&budget)) {
            Ok(()) => { admitted += 1; assert!(prototype.eager); }
            Err(_) => {
                rejected += 1;
                assert!(!prototype.eager);
                assert_eq!(prototype.encode(), before);
                prototype.migrate(ctx).unwrap();
            }
        }
        assert_eq!(unknown.get(), 0);
        prototype.merge_record(&[0x10, 0], ctx).unwrap();
        original.merge_from_slice(&[0x10, 0]).unwrap();
        assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&original.encode_to_vec()).unwrap());
    }
    assert!(rejected > 0 && admitted > 0);
}
#[test]
fn prototype_receiver_matches_owned_codec_and_future_reader_with_no_decode_encoding() {
    let events: &[&[u8]] = &[
        &[0x3a, 2, 8, 1], &[0x3a, 2, 16, 2], &[0x1a, 1, b'a'],
        &[0x3a, 0], &[0x3a, 2, 8, 0], &[0x3a, 6, 0x18, 1, 0x1a, 2, 2, 3],
        &[0x3a, 3, 0x20, 0x81, 0], &[0x2a, 1, b'b'],
    ];
    for a in events { for b in events { for c in events {
        let unknown = std::cell::Cell::new(usize::MAX);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let wire = [*a, *b, *c].concat();
        let mut prototype = ProjectedRecord::new();
        for record in [*a, *b, *c] { prototype.merge_record(record, ctx).unwrap(); }
        let current = v1::Record::decode_from_slice(&wire).unwrap();
        assert_eq!(prototype.current.choice, current.choice);
        assert_eq!(prototype.encode(), current.encode_to_vec(), "wire {wire:?}");
        assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&wire).unwrap());
        prototype.current.choice = Some(v1::record::Choice::Text("edit".into()));
        let mut edited = current;
        edited.choice = Some(v1::record::Choice::Text("edit".into()));
        assert_eq!(prototype.encode(), edited.encode_to_vec(), "edit {wire:?}");
    } } }
}
#[test]
fn prototype_receiver_preserves_edits_followed_by_new_occurrences() {
    let events: &[&[u8]] = &[
        &[0x3a, 2, 8, 1], &[0x3a, 2, 16, 2], &[0x1a, 1, b'a'],
        &[0x3a, 0], &[0x3a, 2, 8, 0], &[0x2a, 1, b'b'],
    ];
    for a in events { for b in events { for c in events {
        for edit in 0..3 {
            let unknown = std::cell::Cell::new(usize::MAX);
            let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
            let mut prototype = ProjectedRecord::new();
            let mut oracle = v1::Record::default();
            for record in [*a, *b] {
                prototype.merge_record(record, ctx).unwrap();
                oracle.merge_from_slice(record).unwrap();
            }
            let choice = match edit {
                0 => None,
                1 => Some(v1::record::Choice::Text("edit".into())),
                _ => { let mut child = v1::record::Child::default(); child.left = Some(42); child.right = Some(0); Some(v1::record::Choice::Detail(Box::new(child))) },
            };
            prototype.current.choice = choice.clone();
            oracle.choice = choice;
            prototype.merge_record(c, ctx).unwrap();
            oracle.merge_from_slice(c).unwrap();
            assert_eq!(prototype.current.choice, oracle.choice);
            assert_eq!(prototype.encode(), oracle.encode_to_vec(), "edit {edit}, events {a:?} {b:?} {c:?}");
            assert_eq!(v2::Record::decode_from_slice(&prototype.encode()).unwrap(), v2::Record::decode_from_slice(&oracle.encode_to_vec()).unwrap());
        }
    } } }
}
#[test]
fn prototype_receiver_fragmented_repeated_values_fit_linear_budget() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let elements = std::cell::Cell::new(128 * 1024);
    let ctx = evolution_fixture::DecodeContext::new(100, &unknown).with_element_memory(&elements);
    let mut prototype = ProjectedRecord::new();
    prototype.merge_record(&[0x2a, 1, b'b'], ctx).unwrap();
    let records = vec![&[0x3a, 2, 0x18, 1][..]; 512];
    prototype.merge_records(&records, ctx).unwrap();
    let Some(v1::record::Choice::Detail(child)) = &prototype.current.choice else { panic!("detail missing") };
    assert_eq!(child.values.len(), 512);
    let wire = [&[0x2a, 1, b'b'][..], &[0x3a, 2, 0x18, 1].repeat(512)].concat();
    let oracle = v1::Record::decode_from_slice(&wire).unwrap();
    assert_eq!(prototype.encode(), oracle.encode_to_vec());
}
#[test]
fn prototype_receiver_budget_failures_do_not_commit_future_or_known_records() {
    for allowance in 0..512 {
        for record in [&[0x2a, 1, b'b'][..], &[0x3a, 2, 16, 2][..]] {
            let unknown = std::cell::Cell::new(usize::MAX);
            let initial = evolution_fixture::DecodeContext::new(100, &unknown);
            let mut prototype = ProjectedRecord::new();
            prototype.merge_record(&[0x3a, 2, 8, 1], initial).unwrap();
            let before = prototype.encode();
            let budget = std::cell::Cell::new(allowance);
            let ctx = initial.with_element_memory(&budget);
            if prototype.merge_record(record, ctx).is_err() {
                assert_eq!(prototype.encode(), before, "allowance {allowance}, record {record:?}");
                assert!(!prototype.active);
                assert!(prototype.events.is_empty());
                // Retry uses a fresh caller budget, not restored spent quota.
                prototype.merge_record(record, initial).unwrap();
            }
            let oracle = v1::Record::decode_from_slice(&[&[0x3a, 2, 8, 1][..], record].concat()).unwrap();
            assert_eq!(prototype.encode(), oracle.encode_to_vec());
        }
    }
}
#[cfg(feature = "semantic-bench")]
mod total_cost {
    use super::*;
    use divan::black_box;
    fn records(shape: &str) -> Vec<Vec<u8>> {
        let mut child = vec![8, 1, 16, 2, 0x32, 64];
        child.extend([0x5a; 64]);
        let mut known = vec![0x3a, child.len() as u8];
        known.extend(child);
        let future = vec![0x2a, 4, 11, 22, 33, 44];
        if shape == "future_first" { vec![future, known] } else { vec![known, future] }
    }
    fn legacy(records: &[Vec<u8>], operation: &str) -> Vec<u8> {
        let mut current = v1::Record::default();
        let wire: Vec<u8> = records.concat();
        if operation == "fragmented" {
            for record in records { current.merge_from_slice(record).unwrap(); }
        } else { current.merge_from_slice(&wire).unwrap(); }
        match operation {
            "edit" => current.choice = Some(v1::record::Choice::Text("edit".into())),
            "retry" => { assert!(current.merge_from_slice(&[0x3a, 4, 8, 3, 0xc0, 0x3e]).is_err()); current.merge_from_slice(&[0x3a, 0]).unwrap(); },
            "clone" => current = current.clone(),
            "repeat_encode" => { black_box(current.encode_to_vec()); },
            _ => {},
        }
        current.encode_to_vec()
    }
    fn projected(records: &[Vec<u8>], operation: &str) -> Vec<u8> {
        let unknown = std::cell::Cell::new(usize::MAX);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let mut current = ProjectedRecord::new();
        let wire: Vec<u8> = records.concat();
        if operation == "fragmented" {
            for record in records { current.merge_record(record, ctx).unwrap(); }
        } else { current.merge_wire(&wire, ctx).unwrap(); }
        match operation {
            "edit" => current.current.choice = Some(v1::record::Choice::Text("edit".into())),
            "retry" => { assert!(current.merge_record(&[0x3a, 4, 8, 3, 0xc0, 0x3e], ctx).is_err()); current.merge_record(&[0x3a, 0], ctx).unwrap(); },
            "clone" => current = current.clone(),
            "repeat_encode" => { black_box(current.encode()); },
            _ => {},
        }
        current.encode()
    }
    #[divan::bench(args = [("future_first", "untouched"), ("future_first", "edit"), ("future_first", "retry"), ("future_first", "clone"), ("future_first", "repeat_encode"), ("future_first", "fragmented"), ("future_last", "untouched"), ("future_last", "edit"), ("future_last", "retry"), ("future_last", "clone"), ("future_last", "repeat_encode"), ("future_last", "fragmented")])]
    fn semantic(bencher: divan::Bencher, (shape, operation): (&str, &str)) {
        let records = records(shape);
        assert_eq!(projected(&records, operation), legacy(&records, operation));
        bencher.bench(|| black_box(projected(black_box(&records), operation)));
    }
    #[divan::bench(args = [("future_first", "untouched"), ("future_first", "edit"), ("future_first", "retry"), ("future_first", "clone"), ("future_first", "repeat_encode"), ("future_first", "fragmented"), ("future_last", "untouched"), ("future_last", "edit"), ("future_last", "retry"), ("future_last", "clone"), ("future_last", "repeat_encode"), ("future_last", "fragmented")])]
    fn eager(bencher: divan::Bencher, (shape, operation): (&str, &str)) {
        let records = records(shape);
        assert_eq!(projected(&records, operation), legacy(&records, operation));
        bencher.bench(|| black_box(legacy(black_box(&records), operation)));
    }
}
#[test]
fn prototype_scalar_retry_keeps_partial_edits_at_budget_boundaries() {
    for allowance in [0, 128, 400, 432, 512] {
        let unknown = std::cell::Cell::new(usize::MAX);
        let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
        let mut prototype = ProjectedRecord::new();
        let mut oracle = v1::Record::default();
        for record in [&[0x3a, 2, 8, 1][..], &[0x2a, 1, b'b'], &[0x3a, 2, 16, 2]] {
            prototype.merge_record(record, ctx).unwrap();
            oracle.merge_from_slice(record).unwrap();
        }
        let failed = [0x3a, 4, 8, 3, 0xc0, 0x3e];
        assert!(prototype.merge_record(&failed, ctx).is_err());
        assert!(oracle.merge_from_slice(&failed).is_err());
        let before = prototype.encode();
        let budget = std::cell::Cell::new(allowance);
        let empty = [0x3a, 0];
        if prototype.merge_record(&empty, ctx.with_element_memory(&budget)).is_err() {
            assert_eq!(prototype.current.choice, oracle.choice);
            assert_eq!(prototype.encode(), before, "quota {allowance}");
            prototype.merge_record(&empty, ctx).unwrap();
        }
        oracle.merge_from_slice(&empty).unwrap();
        assert_eq!(prototype.encode(), oracle.encode_to_vec());
    }
}
#[test]
fn prototype_receiver_retains_partial_edits_and_retries() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
    let records: &[&[u8]] = &[&[0x3a, 2, 8, 1], &[0x2a, 1, b'b'], &[0x3a, 2, 16, 2]];
    let mut prototype = ProjectedRecord::new();
    let mut oracle = v1::Record::default();
    for record in records { prototype.merge_record(record, ctx).unwrap(); oracle.merge_from_slice(record).unwrap(); }
    let failed = [0x3a, 4, 8, 3, 0xc0, 0x3e];
    assert!(prototype.merge_record(&failed, ctx).is_err());
    assert!(oracle.merge_from_slice(&failed).is_err());
    assert_eq!(prototype.current.choice, oracle.choice);
    assert_eq!(prototype.encode(), oracle.encode_to_vec());
    let empty = [0x3a, 0];
    prototype.merge_record(&empty, ctx).unwrap();
    oracle.merge_from_slice(&empty).unwrap();
    assert_eq!(prototype.encode(), oracle.encode_to_vec());
}

#[test]
fn prototype_root_unknown_edits_survive_migration_and_clear() {
    let unknown = std::cell::Cell::new(100);
    let ctx = evolution_fixture::DecodeContext::new(100, &unknown);
    let wire = [0x3a, 2, 8, 1, 0x2a, 1, b'b', 0x3a, 2, 16, 2];
    let mut prototype = ProjectedRecord::new();
    prototype.merge_records(&[&wire[..4], &wire[4..7], &wire[7..]], ctx).unwrap();
    let mut original = v1::Record::decode_from_slice(&wire).unwrap();
    let added = evolution_fixture::UnknownField { number: 99, data: evolution_fixture::UnknownFieldData::Varint(17) };
    original.__buffa_unknown_fields.push(added.clone());
    prototype.current.__buffa_unknown_fields.push(added);
    assert_eq!(prototype.encode(), original.encode_to_vec());
    let before = prototype.encode();
    let zero = std::cell::Cell::new(0);
    assert!(prototype.migrate(ctx.with_element_memory(&zero)).is_err());
    assert_eq!(prototype.encode(), before);
    assert!(!prototype.eager);
    prototype.migrate(ctx).unwrap();
    assert_eq!(prototype.encode(), original.encode_to_vec());
    original.__buffa_unknown_fields.clear();
    prototype.raw_unknowns_mut(ctx).unwrap().clear();
    assert_eq!(prototype.encode(), original.encode_to_vec());

    let mut prototype = ProjectedRecord::new();
    prototype.merge_records(&[&wire[..4], &wire[4..7], &wire[7..]], ctx).unwrap();
    let mut original = v1::Record::decode_from_slice(&wire).unwrap();
    prototype.raw_unknowns_mut(ctx).unwrap().clear();
    original.__buffa_unknown_fields.clear();
    assert_eq!(prototype.encode(), original.encode_to_vec());
}

}
