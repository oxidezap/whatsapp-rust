use evolution_fixture::{Message, MessageView, ViewEncode};

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
