use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

fn mk_patch(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode");
    Patch::from_binary(&bytes).expect("decode")
}

#[test]
fn upstream_port_patch_rebase_rewrites_op_ids() {
    let sid = 1;
    let patch = mk_patch(
        sid,
        5,
        &[DecodedOp::InsArr {
            id: Timestamp { sid, time: 5 },
            obj: Timestamp { sid, time: 3 },
            reference: Timestamp { sid, time: 3 },
            data: vec![Timestamp { sid: 0, time: 10 }],
        }],
    );
    let rebased = patch.rebase(10, Some(5)).expect("rebase");
    assert_eq!(rebased.decoded_ops()[0].id().time, 10);
}

#[test]
fn upstream_port_patch_rebase_does_not_rewrite_old_server_refs() {
    let sid = 1;
    let patch = mk_patch(
        sid,
        5,
        &[DecodedOp::InsArr {
            id: Timestamp { sid, time: 5 },
            obj: Timestamp { sid, time: 3 },
            reference: Timestamp { sid, time: 3 },
            data: vec![Timestamp { sid: 0, time: 10 }],
        }],
    );
    let rebased = patch.rebase(10, Some(5)).expect("rebase");
    match &rebased.decoded_ops()[0] {
        DecodedOp::InsArr { reference, .. } => assert_eq!(reference.time, 3),
        other => panic!("expected ins_arr, got {other:?}"),
    }
}

#[test]
fn upstream_port_patch_rebase_rewrites_new_refs_in_horizon() {
    let sid = 1;
    let patch = mk_patch(
        sid,
        5,
        &[DecodedOp::InsArr {
            id: Timestamp { sid, time: 5 },
            obj: Timestamp { sid, time: 7 },
            reference: Timestamp { sid, time: 7 },
            data: vec![Timestamp { sid: 0, time: 10 }],
        }],
    );
    let rebased = patch.rebase(10, Some(5)).expect("rebase");
    match &rebased.decoded_ops()[0] {
        DecodedOp::InsArr { reference, .. } => assert_eq!(reference.time, 12),
        other => panic!("expected ins_arr, got {other:?}"),
    }
}

#[test]
fn upstream_port_patch_rebase_transforms_timestamp_con_values_on_same_sid() {
    let sid = 500_001;
    let patch = mk_patch(
        sid,
        20,
        &[DecodedOp::NewCon {
            id: Timestamp { sid, time: 20 },
            value: ConValue::Ref(Timestamp { sid, time: 25 }),
        }],
    );
    let rebased = patch.rebase(1000, None).expect("rebase");
    match &rebased.decoded_ops()[0] {
        DecodedOp::NewCon {
            value: ConValue::Ref(ts),
            ..
        } => assert_eq!(ts.time, 1005),
        other => panic!("expected new_con timestamp ref, got {other:?}"),
    }
}
