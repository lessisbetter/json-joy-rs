use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream references:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/nodes/str/__tests__/StrNode.spec.ts
//   - "append inserts by concurrent users"
//   - "one user merging chunk, while another synchronously inserting at the same position"
//   - "one user merging chunk, while another synchronously inserting at the same position - 2"

fn apply_one(runtime: &mut RuntimeModel, op: DecodedOp) {
    let id = op.id();
    let bytes = encode_patch_from_ops(id.sid, id.time, &[op]).expect("single-op patch encode must succeed");
    let patch = Patch::from_binary(&bytes).expect("single-op patch decode must succeed");
    runtime
        .apply_patch(&patch)
        .expect("single-op patch apply must succeed");
}

fn new_string_root_model() -> (RuntimeModel, Timestamp) {
    let mut runtime = RuntimeModel::new_logical_empty(90001);
    let str_id = Timestamp {
        sid: 90001,
        time: 1,
    };
    let init_ops = vec![
        DecodedOp::NewStr { id: str_id },
        DecodedOp::InsVal {
            id: Timestamp {
                sid: 90001,
                time: 2,
            },
            obj: Timestamp { sid: 0, time: 0 },
            val: str_id,
        },
    ];
    let bytes = encode_patch_from_ops(90001, 1, &init_ops).expect("init patch encode must succeed");
    let patch = Patch::from_binary(&bytes).expect("init patch decode must succeed");
    runtime
        .apply_patch(&patch)
        .expect("init patch apply must succeed");
    (runtime, str_id)
}

#[test]
fn upstream_port_str_append_concurrent_order_independent() {
    let (mut m1, str_id) = new_string_root_model();
    let (mut m2, _) = new_string_root_model();

    // Equivalent to upstream scenario "append inserts by concurrent users".
    let ops_order_1 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 3 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 2 },
            data: "1".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 3 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 2 },
            data: "2".to_string(),
        },
    ];
    let ops_order_2 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 3 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 2 },
            data: "2".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 3 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 2 },
            data: "1".to_string(),
        },
    ];

    for op in ops_order_1 {
        apply_one(&mut m1, op);
    }
    for op in ops_order_2 {
        apply_one(&mut m2, op);
    }

    assert_eq!(m1.view_json(), serde_json::json!("aa21"));
    assert_eq!(m2.view_json(), serde_json::json!("aa21"));
}

#[test]
fn upstream_port_str_sync_insert_same_position_order_independent() {
    let (mut m1, str_id) = new_string_root_model();
    let (mut m2, _) = new_string_root_model();

    // Equivalent to upstream scenario:
    // "one user merging chunk, while another synchronously inserting at the same position".
    let ops_order_1 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "1".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "2".to_string(),
        },
    ];
    let ops_order_2 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "2".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 1, time: 1 },
            data: "1".to_string(),
        },
    ];

    for op in ops_order_1 {
        apply_one(&mut m1, op);
    }
    for op in ops_order_2 {
        apply_one(&mut m2, op);
    }

    assert_eq!(m1.view_json(), serde_json::json!("a21"));
    assert_eq!(m2.view_json(), serde_json::json!("a21"));
}

#[test]
fn upstream_port_str_chunk_merge_with_sync_insert_order_independent() {
    let (mut m1, str_id) = new_string_root_model();
    let (mut m2, _) = new_string_root_model();

    // Equivalent to upstream scenario:
    // "one user merging chunk, while another synchronously inserting at the same position - 2".
    let ops_order_1 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 2, time: 1 },
            data: "12345".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 2, time: 1 },
            data: "x".to_string(),
        },
    ];
    let ops_order_2 = vec![
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 1 },
            obj: str_id,
            reference: str_id,
            data: "a".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 1, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 2, time: 1 },
            data: "x".to_string(),
        },
        DecodedOp::InsStr {
            id: Timestamp { sid: 2, time: 2 },
            obj: str_id,
            reference: Timestamp { sid: 2, time: 1 },
            data: "12345".to_string(),
        },
    ];

    for op in ops_order_1 {
        apply_one(&mut m1, op);
    }
    for op in ops_order_2 {
        apply_one(&mut m2, op);
    }

    assert_eq!(m1.view_json(), serde_json::json!("a12345x"));
    assert_eq!(m2.view_json(), serde_json::json!("a12345x"));
}
