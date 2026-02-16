use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use json_joy_core::patch_compaction::{combine_patches, compact_patch, CompactionError};

fn mk_patch(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode patch");
    Patch::from_binary(&bytes).expect("decode patch")
}

#[test]
fn upstream_port_patch_combine_adjacent_and_gapped() {
    let sid = 123;
    let p1_ops = vec![
        DecodedOp::NewStr {
            id: Timestamp { sid, time: 1 },
        },
        DecodedOp::InsStr {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid, time: 1 },
            reference: Timestamp { sid, time: 1 },
            data: "a".to_string(),
        },
    ];
    let p2_adj_ops = vec![DecodedOp::NewCon {
        id: Timestamp { sid, time: 3 },
        value: ConValue::Json(serde_json::json!(1)),
    }];
    let p2_gap_ops = vec![DecodedOp::NewCon {
        id: Timestamp { sid, time: 10 },
        value: ConValue::Json(serde_json::json!(2)),
    }];

    let p1 = mk_patch(sid, 1, &p1_ops);
    let p2_adj = mk_patch(sid, 3, &p2_adj_ops);
    let p2_gap = mk_patch(sid, 10, &p2_gap_ops);

    let combined_adj = combine_patches(&[p1.clone(), p2_adj]).expect("combine adjacent");
    assert_eq!(combined_adj.decoded_ops().len(), 3);
    assert!(matches!(
        combined_adj.decoded_ops()[2],
        DecodedOp::NewCon { .. }
    ));

    let combined_gap = combine_patches(&[p1, p2_gap]).expect("combine gap");
    assert_eq!(combined_gap.decoded_ops().len(), 4);
    match &combined_gap.decoded_ops()[2] {
        DecodedOp::Nop { id, len } => {
            assert_eq!(*id, Timestamp { sid, time: 3 });
            assert_eq!(*len, 7);
        }
        other => panic!("expected nop between patches, got {other:?}"),
    }
}

#[test]
fn upstream_port_patch_combine_sid_mismatch_rejected() {
    let p1 = mk_patch(
        111,
        1,
        &[DecodedOp::NewCon {
            id: Timestamp { sid: 111, time: 1 },
            value: ConValue::Json(serde_json::json!(1)),
        }],
    );
    let p2 = mk_patch(
        222,
        2,
        &[DecodedOp::NewCon {
            id: Timestamp { sid: 222, time: 2 },
            value: ConValue::Json(serde_json::json!(2)),
        }],
    );
    let err = combine_patches(&[p1, p2]).expect_err("sid mismatch must fail");
    assert!(matches!(err, CompactionError::SidMismatch));
}

#[test]
fn upstream_port_patch_compact_merges_consecutive_ins_str_appends() {
    let sid = 77;
    let patch = mk_patch(
        sid,
        1,
        &[
            DecodedOp::NewStr {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 1 },
                data: "hello".to_string(),
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 7 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 6 },
                data: " world".to_string(),
            },
        ],
    );
    let compacted = compact_patch(&patch).expect("compact");
    assert_eq!(compacted.decoded_ops().len(), 2);
    match &compacted.decoded_ops()[1] {
        DecodedOp::InsStr { data, .. } => assert_eq!(data, "hello world"),
        other => panic!("expected ins_str, got {other:?}"),
    }
}

#[test]
fn upstream_port_patch_compact_does_not_merge_non_append() {
    let sid = 88;
    let patch = mk_patch(
        sid,
        1,
        &[
            DecodedOp::NewStr {
                id: Timestamp { sid, time: 1 },
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 2 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 1 },
                data: "a".to_string(),
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 3 },
                obj: Timestamp { sid, time: 1 },
                reference: Timestamp { sid, time: 99 },
                data: "b".to_string(),
            },
        ],
    );
    let compacted = compact_patch(&patch).expect("compact");
    assert_eq!(compacted.decoded_ops().len(), 3);
}
