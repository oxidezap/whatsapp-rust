use evolution_fixture::{Message, MessageView, ViewEncode};
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
