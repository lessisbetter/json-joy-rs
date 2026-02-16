use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream reference:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/nodes/val/ValNode.ts
//   - set(val): ignore when val <= current (for non-system current)
//   - set(val): ignore when val <= register id

fn apply_ops(runtime: &mut RuntimeModel, sid: u64, time: u64, ops: Vec<DecodedOp>) {
    let bytes = encode_patch_from_ops(sid, time, &ops).expect("patch encode must succeed");
    let patch = Patch::from_binary(&bytes).expect("patch decode must succeed");
    runtime
        .apply_patch(&patch)
        .expect("patch apply must succeed");
}

#[test]
fn upstream_port_val_lww_ignores_stale_foreign_value() {
    let mut runtime = RuntimeModel::new_logical_empty(100);
    let reg = Timestamp { sid: 100, time: 1 };

    apply_ops(
        &mut runtime,
        100,
        1,
        vec![
            DecodedOp::NewVal { id: reg },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: reg,
            },
            DecodedOp::NewCon {
                id: Timestamp { sid: 100, time: 3 },
                value: ConValue::Json(serde_json::json!("A")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 4 },
                obj: reg,
                val: Timestamp { sid: 100, time: 3 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid: 100, time: 5 },
                value: ConValue::Json(serde_json::json!("B")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 6 },
                obj: reg,
                val: Timestamp { sid: 100, time: 5 },
            },
        ],
    );

    // Foreign value timestamp (50,1) is older than current (100,5) by time.
    apply_ops(
        &mut runtime,
        50,
        1,
        vec![
            DecodedOp::NewCon {
                id: Timestamp { sid: 50, time: 1 },
                value: ConValue::Json(serde_json::json!("STALE")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 50, time: 2 },
                obj: reg,
                val: Timestamp { sid: 50, time: 1 },
            },
        ],
    );

    assert_eq!(runtime.view_json(), serde_json::json!("B"));
}

#[test]
fn upstream_port_val_lww_rejects_value_not_newer_than_register_id() {
    let mut runtime = RuntimeModel::new_logical_empty(100);
    let reg = Timestamp { sid: 100, time: 1 };

    apply_ops(
        &mut runtime,
        100,
        1,
        vec![
            DecodedOp::NewVal { id: reg },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: reg,
            },
        ],
    );

    // Value id (50,1) is <= register id (100,1) by compare(time,sid), so ignore.
    apply_ops(
        &mut runtime,
        50,
        1,
        vec![
            DecodedOp::NewCon {
                id: Timestamp { sid: 50, time: 1 },
                value: ConValue::Json(serde_json::json!("TOO_OLD_FOR_REG")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 50, time: 2 },
                obj: reg,
                val: Timestamp { sid: 50, time: 1 },
            },
        ],
    );

    assert_eq!(runtime.view_json(), serde_json::Value::Null);
}

#[test]
fn upstream_port_val_lww_accepts_newer_foreign_value() {
    let mut runtime = RuntimeModel::new_logical_empty(100);
    let reg = Timestamp { sid: 100, time: 1 };

    apply_ops(
        &mut runtime,
        100,
        1,
        vec![
            DecodedOp::NewVal { id: reg },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 2 },
                obj: Timestamp { sid: 0, time: 0 },
                val: reg,
            },
            DecodedOp::NewCon {
                id: Timestamp { sid: 100, time: 3 },
                value: ConValue::Json(serde_json::json!("A")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 100, time: 4 },
                obj: reg,
                val: Timestamp { sid: 100, time: 3 },
            },
        ],
    );

    // Foreign value id (200,10) is newer than current (100,3) and register id (100,1).
    apply_ops(
        &mut runtime,
        200,
        10,
        vec![
            DecodedOp::NewCon {
                id: Timestamp { sid: 200, time: 10 },
                value: ConValue::Json(serde_json::json!("NEWER")),
            },
            DecodedOp::InsVal {
                id: Timestamp { sid: 200, time: 11 },
                obj: reg,
                val: Timestamp { sid: 200, time: 10 },
            },
        ],
    );

    assert_eq!(runtime.view_json(), serde_json::json!("NEWER"));
}
