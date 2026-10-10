#![allow(clippy::disallowed_methods)]
#[path = "fixtures/semantic_projection.rs"]
mod semantic;
use semantic::{Arena, Field, Kind};
const ROOT: &[Field] = &[
    Field {
        number: 1,
        kind: Kind::Bytes,
        repeated: false,
        oneof: 0,
    },
    Field {
        number: 6,
        kind: Kind::Message(1),
        repeated: false,
        oneof: 0,
    },
    Field {
        number: 3,
        kind: Kind::Bytes,
        repeated: false,
        oneof: 1,
    },
    Field {
        number: 7,
        kind: Kind::Message(1),
        repeated: false,
        oneof: 1,
    },
];
const CHILD: &[Field] = &[
    Field {
        number: 1,
        kind: Kind::Uint32,
        repeated: false,
        oneof: 0,
    },
    Field {
        number: 2,
        kind: Kind::Uint32,
        repeated: false,
        oneof: 0,
    },
    Field {
        number: 3,
        kind: Kind::Uint32,
        repeated: true,
        oneof: 0,
    },
    Field {
        number: 32,
        kind: Kind::Uint32,
        repeated: true,
        oneof: 0,
    },
];
const SCHEMA: &[&[Field]] = &[ROOT, CHILD];
fn context(unknown: &std::cell::Cell<usize>) -> buffa::DecodeContext<'_> {
    buffa::DecodeContext::new(100, unknown)
}
#[test]
fn projection_merges_old_semantics_and_keeps_partial_failure_separate() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 2, 8, 1], context(&unknown))
        .unwrap();
    baseline
        .accept(&[0x3a, 2, 16, 2], context(&unknown))
        .unwrap();
    assert_eq!(baseline.encode_calls(), 0);
    // No encoder or generated owner is called while constructing the baseline.
    assert_eq!(baseline.encode(), [0x3a, 4, 8, 1, 16, 2]);
    let before = baseline.encode();
    assert!(
        baseline
            .accept(&[0x3a, 4, 8, 3, 0xc0, 0x3e], context(&unknown))
            .is_err()
    );
    assert_eq!(baseline.encode(), before);
    baseline.accept(&[0x3a, 0], context(&unknown)).unwrap();
    assert_eq!(baseline.encode(), before);
}
#[test]
fn projection_preserves_presence_and_mixed_packing() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(
            &[0x3a, 11, 8, 0x80, 0, 0x18, 1, 0x1a, 2, 2, 3, 16, 0],
            context(&unknown),
        )
        .unwrap();
    assert_eq!(
        baseline.encode(),
        [0x3a, 10, 8, 0, 16, 0, 0x18, 1, 0x18, 2, 0x18, 3]
    );
}
#[test]
fn failed_projection_respects_budget_and_retains_prior_values() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 2, 8, 1], context(&unknown))
        .unwrap();
    let expected = baseline.encode();
    let budget = std::cell::Cell::new(0);
    assert!(
        baseline
            .accept(
                &[0x3a, 2, 16, 2],
                context(&unknown).with_element_memory(&budget)
            )
            .is_err()
    );
    assert_eq!(baseline.encode(), expected);
    baseline
        .accept(&[0x3a, 2, 16, 2], context(&unknown))
        .unwrap();
    assert_eq!(baseline.encode(), [0x3a, 4, 8, 1, 16, 2]);
}

#[test]
fn descriptor_is_compact_and_all_truncations_leave_the_baseline_untouched() {
    assert!(size_of::<Field>() <= 12);
    let unknown = std::cell::Cell::new(usize::MAX);
    let valid = [0x3a, 6, 8, 3, 0x1a, 2, 4, 5];
    for boundary in 1..valid.len() {
        let mut projection = Arena::new(SCHEMA);
        projection
            .accept(&[0x3a, 2, 16, 2], context(&unknown))
            .unwrap();
        let before = projection.encode();
        assert!(
            projection
                .accept(&valid[..boundary], context(&unknown))
                .is_err(),
            "boundary {boundary}"
        );
        assert_eq!(projection.encode(), before);
        projection.accept(&valid, context(&unknown)).unwrap();
        assert_eq!(
            projection.encode(),
            [0x3a, 8, 8, 3, 16, 2, 0x18, 4, 0x18, 5]
        );
    }
}

struct ChildReceiver {
    left: Option<u32>,
    right: Option<u32>,
}
impl semantic::Visitor for ChildReceiver {
    fn field(&self, number: u32, index: usize) -> Option<semantic::ValueRef<'_>> {
        if index != 0 {
            return None;
        }
        match number {
            1 => self.left,
            2 => self.right,
            _ => None,
        }
        .map(|v| semantic::ValueRef::Varint(u64::from(v)))
    }
}
struct Receiver {
    child: ChildReceiver,
}
impl semantic::Visitor for Receiver {
    fn field(&self, number: u32, index: usize) -> Option<semantic::ValueRef<'_>> {
        (number == 7 && index == 0).then_some(semantic::ValueRef::Child(&self.child))
    }
}
#[test]
fn direct_receiver_comparison_never_encodes_or_blesses_partial_mutation() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 2, 8, 1], context(&unknown))
        .unwrap();
    baseline
        .accept(&[0x3a, 2, 16, 2], context(&unknown))
        .unwrap();
    let mut current = Receiver {
        child: ChildReceiver {
            left: Some(1),
            right: Some(2),
        },
    };
    assert!(baseline.matches(&current));
    current.child.left = Some(3);
    assert!(!baseline.matches(&current));
    assert!(baseline.matches(&Receiver {
        child: ChildReceiver {
            left: Some(1),
            right: Some(2)
        }
    }));
    assert_eq!(baseline.encode_calls(), 0);
    let direct = Arena::from_visitor(
        SCHEMA,
        &Receiver {
            child: ChildReceiver {
                left: Some(1),
                right: Some(2),
            },
        },
        context(&unknown),
    )
    .unwrap();
    assert_eq!(direct.encode_calls(), 0);
    assert_eq!(direct.encode(), baseline.encode());
    // The generic visitor also represents bytes/fixed unknown data. Groups
    // remain an explicit unsupported case in this limited fixture closure.
    let _ = [
        semantic::ValueRef::Bytes(b"x"),
        semantic::ValueRef::Fixed32(1),
        semantic::ValueRef::Fixed64(2),
        semantic::ValueRef::Unsupported,
    ];
}

#[test]
fn abandoning_a_prepared_record_retains_completed_baseline() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 2, 8, 1], context(&unknown))
        .unwrap();
    let before = baseline.encode();
    drop(
        baseline
            .stage(&[0x3a, 2, 16, 2], context(&unknown))
            .unwrap(),
    );
    assert_eq!(baseline.encode(), before);
    baseline
        .stage(&[0x3a, 2, 16, 2], context(&unknown))
        .unwrap()
        .commit();
    assert_eq!(baseline.encode(), [0x3a, 4, 8, 1, 16, 2]);
    assert_eq!(
        baseline
            .clone_with_context(context(&unknown))
            .unwrap()
            .encode(),
        baseline.encode()
    );
}

#[test]
fn raw_records_share_stable_offsets_without_becoming_canonical_baselines() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    assert!(baseline.is_empty());
    let raw = [0x3a, 3, 8, 0x81, 0];
    let range = baseline.stage(&raw, context(&unknown)).unwrap().commit();
    assert_eq!(baseline.record(&range), raw);
    assert!(
        baseline
            .stage(&[0x3a, 4, 8, 3, 0xc0, 0x3e], context(&unknown))
            .is_err()
    );
    assert_eq!(baseline.record(&range), raw);
    assert_eq!(baseline.encode_calls(), 0);
    // The ordered tape replays the original overlong occurrence; the semantic
    // prefix encodes canonically only when output is requested.
    assert_eq!(baseline.encode(), [0x3a, 2, 8, 1]);
    baseline
        .accept(&[0x1a, 1, b'a'], context(&unknown))
        .unwrap();
    assert_eq!(baseline.record(&range), raw);
    assert_eq!(baseline.encode(), [0x1a, 1, b'a']);
}

#[test]
fn empty_projection_needs_no_unbudgeted_root_allocation() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    let budget = std::cell::Cell::new(4);
    assert!(
        baseline
            .stage(
                &[0x3a, 2, 8, 1],
                context(&unknown).with_element_memory(&budget)
            )
            .is_err()
    );
    assert!(baseline.is_empty());
    assert_eq!(baseline.encode_calls(), 0);
    assert!(baseline.encode().is_empty());
    baseline
        .accept(&[0x3a, 2, 8, 1], context(&unknown))
        .unwrap();
    assert_eq!(baseline.encode(), [0x3a, 2, 8, 1]);
}

#[test]
fn single_scalar_reconciliation_is_prepared_before_publication() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 4, 8, 1, 16, 2], context(&unknown))
        .unwrap();
    let current = Receiver {
        child: ChildReceiver {
            left: Some(3),
            right: Some(2),
        },
    };
    let edit = baseline.scalar_edit(&current).unwrap();
    assert_eq!(baseline.encode_calls(), 0);
    let budget = std::cell::Cell::new(0);
    assert!(
        baseline
            .clone_with_context(context(&unknown).with_element_memory(&budget))
            .is_err()
    );
    assert!(!baseline.matches(&current));
    let mut prefix = baseline.clone_with_context(context(&unknown)).unwrap();
    prefix.apply_scalar(edit.clone());
    prefix.compact_projection();
    assert!(prefix.matches(&current));
    assert!(!baseline.matches(&current));
    baseline.apply_scalar(edit);
    baseline.compact_projection();
    assert!(baseline.matches(&current));
    assert!(
        baseline
            .scalar_edit(&Receiver {
                child: ChildReceiver {
                    left: Some(4),
                    right: Some(5)
                }
            })
            .is_none()
    );
    assert!(
        baseline
            .scalar_edit(&Receiver {
                child: ChildReceiver {
                    left: None,
                    right: Some(2)
                }
            })
            .is_none()
    );
    baseline
        .accept(&[0x3a, 2, 8, 4], context(&unknown))
        .unwrap();
    assert_eq!(prefix.encode(), [0x3a, 4, 8, 3, 16, 2]);
    assert_eq!(baseline.encode(), [0x3a, 4, 8, 4, 16, 2]);
}

#[test]
fn projection_compaction_preserves_payloads_and_remaps_live_children() {
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    for record in [
        &[0x32, 5, 0x22, 3, b'a', b'b', b'c'][..],
        &[0x3a, 2, 8, 1],
        &[0x1a, 1, b'z'],
        &[0x3a, 2, 16, 2],
    ] {
        baseline.accept(record, context(&unknown)).unwrap();
    }
    let before = baseline.encode();
    assert!(baseline.encoding_bound() >= before.len());
    baseline.compact_projection();
    assert_eq!(baseline.encode(), before);
    baseline
        .accept(&[0x3a, 2, 8, 3], context(&unknown))
        .unwrap();
    assert_eq!(
        baseline.encode(),
        [0x32, 5, 0x22, 3, b'a', b'b', b'c', 0x3a, 4, 8, 3, 16, 2]
    );
}

#[test]
fn compaction_stack_bounds_preserve_larger_graphs_and_payload_lists() {
    const MANY_CHILDREN: &[&[Field]] = &[
        &[Field {
            number: 1,
            kind: Kind::Message(1),
            repeated: true,
            oneof: 0,
        }],
        &[Field {
            number: 1,
            kind: Kind::Bytes,
            repeated: false,
            oneof: 0,
        }],
    ];
    const MANY_BYTES: &[&[Field]] = &[&[Field {
        number: 1,
        kind: Kind::Bytes,
        repeated: true,
        oneof: 0,
    }]];
    let unknown = std::cell::Cell::new(usize::MAX);
    for (schema, record) in [
        (MANY_CHILDREN, &[0x0a, 3, 0x0a, 1, b'a'][..]),
        (MANY_BYTES, &[0x0a, 1, b'a'][..]),
    ] {
        for count in [15, 16, 17] {
            let mut baseline = Arena::new(schema);
            for _ in 0..count {
                baseline.accept(record, context(&unknown)).unwrap();
            }
            let before = baseline.encode();
            baseline.compact_projection();
            assert_eq!(baseline.encode(), before);
            baseline.accept(record, context(&unknown)).unwrap();
            assert_eq!(baseline.encode(), [before, record.to_vec()].concat());
        }
    }
}

#[test]
fn compaction_remaps_breadth_first_nodes_created_in_depth_first_order() {
    const TREE: &[&[Field]] = &[
        &[Field {
            number: 1,
            kind: Kind::Message(1),
            repeated: true,
            oneof: 0,
        }],
        &[
            Field {
                number: 1,
                kind: Kind::Message(2),
                repeated: false,
                oneof: 0,
            },
            Field {
                number: 2,
                kind: Kind::Bytes,
                repeated: false,
                oneof: 0,
            },
        ],
        &[Field {
            number: 1,
            kind: Kind::Bytes,
            repeated: false,
            oneof: 0,
        }],
    ];
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(TREE);
    let record = [
        0x0a, 8, 0x0a, 3, 0x0a, 1, b'a', 0x12, 1, b'b', 0x0a, 3, 0x12, 1, b'c',
    ];
    baseline.accept(&record, context(&unknown)).unwrap();
    let before = baseline.encode();
    baseline.compact_projection();
    assert_eq!(baseline.encode(), before);
    let next = [0x0a, 3, 0x12, 1, b'd'];
    baseline.accept(&next, context(&unknown)).unwrap();
    assert_eq!(baseline.encode(), [before, next.to_vec()].concat());
    baseline.compact_projection();
    assert_eq!(baseline.encode(), [record.to_vec(), next.to_vec()].concat());
}

#[test]
fn scalar_plan_tracks_one_repeated_element_and_rejects_two_changes() {
    struct List([u64; 3]);
    impl semantic::Visitor for List {
        fn field(&self, number: u32, index: usize) -> Option<semantic::ValueRef<'_>> {
            (number == 3)
                .then(|| self.0.get(index))
                .flatten()
                .copied()
                .map(semantic::ValueRef::Varint)
        }
    }
    struct RootList(List);
    impl semantic::Visitor for RootList {
        fn field(&self, number: u32, index: usize) -> Option<semantic::ValueRef<'_>> {
            (number == 7 && index == 0).then_some(semantic::ValueRef::Child(&self.0))
        }
    }
    let unknown = std::cell::Cell::new(usize::MAX);
    let mut baseline = Arena::new(SCHEMA);
    baseline
        .accept(&[0x3a, 5, 0x1a, 3, 1, 2, 3], context(&unknown))
        .unwrap();
    assert!(baseline.scalar_edit(&RootList(List([4, 5, 3]))).is_none());
    assert!(baseline.scalar_edit(&RootList(List([1, 2, 3]))).is_none());
    let current = RootList(List([1, 4, 3]));
    let edit = baseline.scalar_edit(&current).unwrap();
    baseline.apply_scalar(edit);
    assert!(baseline.matches(&current));
    assert_eq!(baseline.encode_calls(), 0);
    baseline.compact_projection();
    assert_eq!(baseline.encode(), [0x3a, 6, 0x18, 1, 0x18, 4, 0x18, 3]);
}
