use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timespan, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use serde_json::json;

fn ts(sid: u64, time: u64) -> Timestamp {
    Timestamp { sid, time }
}

fn span(sid: u64, time: u64, len: u64) -> Timespan {
    Timespan {
        sid,
        time,
        span: len,
    }
}

fn apply_ops(runtime: &mut RuntimeModel, sid: u64, time: u64, ops: &[DecodedOp]) {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode patch");
    let patch = Patch::from_binary(&bytes).expect("decode patch");
    runtime.apply_patch(&patch).expect("apply patch");
}

fn apply_one(runtime: &mut RuntimeModel, op: DecodedOp) {
    let id = op.id();
    apply_ops(runtime, id.sid, id.time, &[op]);
}

fn arr_root_runtime() -> (RuntimeModel, Timestamp) {
    let sid = 99120;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let arr = ts(sid, 1);
    let ops = vec![
        DecodedOp::NewArr { id: arr },
        DecodedOp::InsVal {
            id: ts(sid, 2),
            obj: ts(0, 0),
            val: arr,
        },
    ];
    apply_ops(&mut runtime, sid, 1, &ops);
    (runtime, arr)
}

fn bin_root_runtime() -> (RuntimeModel, Timestamp) {
    let sid = 99121;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let bin = ts(sid, 1);
    let ops = vec![
        DecodedOp::NewBin { id: bin },
        DecodedOp::InsVal {
            id: ts(sid, 2),
            obj: ts(0, 0),
            val: bin,
        },
    ];
    apply_ops(&mut runtime, sid, 1, &ops);
    (runtime, bin)
}

#[test]
fn upstream_port_arr_rga_matrix_core_behaviors() {
    // Upstream reference:
    // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.array.spec.ts
    let (mut runtime, arr) = arr_root_runtime();
    let sid = 99120;
    let t = ts(sid, 10);
    let f = ts(sid, 11);
    let n = ts(sid, 12);
    let ops = vec![
        DecodedOp::NewCon {
            id: t,
            value: ConValue::Json(json!(true)),
        },
        DecodedOp::NewCon {
            id: f,
            value: ConValue::Json(json!(false)),
        },
        DecodedOp::NewCon {
            id: n,
            value: ConValue::Json(serde_json::Value::Null),
        },
        DecodedOp::InsArr {
            id: ts(sid, 20),
            obj: arr,
            reference: arr,
            data: vec![t, t],
        },
        DecodedOp::InsArr {
            id: ts(sid, 30),
            obj: arr,
            reference: ts(sid, 21),
            data: vec![f, n],
        },
        DecodedOp::InsArr {
            id: ts(sid, 40),
            obj: arr,
            reference: ts(sid, 21),
            data: vec![n],
        },
    ];
    for op in ops.clone() {
        apply_one(&mut runtime, op);
    }
    assert_eq!(
        runtime.view_json(),
        json!([
            true,
            true,
            serde_json::Value::Null,
            false,
            serde_json::Value::Null
        ])
    );

    // Same patch replayed multiple times should be idempotent.
    for op in ops.clone() {
        apply_one(&mut runtime, op);
    }
    for op in ops {
        apply_one(&mut runtime, op);
    }
    assert_eq!(
        runtime.view_json(),
        json!([
            true,
            true,
            serde_json::Value::Null,
            false,
            serde_json::Value::Null
        ])
    );
}

#[test]
fn upstream_port_arr_rga_delete_across_split_chunks() {
    // Upstream reference:
    // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.array.spec.ts
    // "can delete across chunk when chunk were split due to insertion"
    let (mut runtime, arr) = arr_root_runtime();
    let sid = 99122;
    let t1 = ts(sid, 10);
    let t2 = ts(sid, 11);
    let t3 = ts(sid, 12);
    let f1 = ts(sid, 13);
    let f2 = ts(sid, 14);
    let ops = vec![
        DecodedOp::NewCon {
            id: t1,
            value: ConValue::Json(json!(true)),
        },
        DecodedOp::NewCon {
            id: t2,
            value: ConValue::Json(json!(true)),
        },
        DecodedOp::NewCon {
            id: t3,
            value: ConValue::Json(json!(true)),
        },
        DecodedOp::NewCon {
            id: f1,
            value: ConValue::Json(json!(false)),
        },
        DecodedOp::NewCon {
            id: f2,
            value: ConValue::Json(json!(false)),
        },
        DecodedOp::InsArr {
            id: ts(sid, 20),
            obj: arr,
            reference: arr,
            data: vec![t1, t2, t3],
        },
        DecodedOp::InsArr {
            id: ts(sid, 30),
            obj: arr,
            reference: ts(sid, 21),
            data: vec![f1, f2],
        },
        DecodedOp::Del {
            id: ts(sid, 40),
            obj: arr,
            what: vec![span(sid, 21, 2)],
        },
    ];
    for op in ops {
        apply_one(&mut runtime, op);
    }
    assert_eq!(runtime.view_json(), json!([true, false, false]));
}

#[test]
fn upstream_port_bin_rga_matrix_core_behaviors() {
    // Upstream reference:
    // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.binary.spec.ts
    let (mut runtime, bin) = bin_root_runtime();
    let sid = 99121;
    let ops = vec![
        DecodedOp::InsBin {
            id: ts(sid, 10),
            obj: bin,
            reference: bin,
            data: vec![1, 2],
        },
        DecodedOp::InsBin {
            id: ts(sid, 12),
            obj: bin,
            reference: ts(sid, 11),
            data: vec![3, 4],
        },
        DecodedOp::InsBin {
            id: ts(sid, 14),
            obj: bin,
            reference: ts(sid, 11),
            data: vec![5],
        },
    ];
    for op in ops.clone() {
        apply_one(&mut runtime, op);
    }
    assert_eq!(runtime.view_json(), json!({"0":1,"1":2,"2":5,"3":3,"4":4}));

    // Apply same patch repeatedly: idempotent.
    for op in ops.clone() {
        apply_one(&mut runtime, op);
    }
    for op in ops {
        apply_one(&mut runtime, op);
    }
    assert_eq!(runtime.view_json(), json!({"0":1,"1":2,"2":5,"3":3,"4":4}));
}

#[test]
fn upstream_port_bin_rga_delete_across_chunks() {
    // Upstream reference:
    // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.binary.spec.ts
    // "can delete across chunks"
    let (mut runtime, bin) = bin_root_runtime();
    let sid = 99123;
    let ops = vec![
        DecodedOp::InsBin {
            id: ts(sid, 10),
            obj: bin,
            reference: bin,
            data: vec![1, 2, 3, 4, 5],
        },
        DecodedOp::InsBin {
            id: ts(sid, 15),
            obj: bin,
            reference: ts(sid, 14),
            data: vec![6],
        },
        DecodedOp::InsBin {
            id: ts(sid, 16),
            obj: bin,
            reference: ts(sid, 15),
            data: vec![7, 8, 9, 10, 11, 12],
        },
        DecodedOp::InsBin {
            id: ts(sid, 22),
            obj: bin,
            reference: ts(sid, 21),
            data: vec![13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25],
        },
        DecodedOp::Del {
            id: ts(sid, 60),
            obj: bin,
            what: vec![span(sid, 13, 11)],
        },
    ];
    for op in ops {
        apply_one(&mut runtime, op);
    }
    assert_eq!(
        runtime.view_json(),
        json!({"0":1,"1":2,"2":3,"3":15,"4":16,"5":17,"6":18,"7":19,"8":20,"9":21,"10":22,"11":23,"12":24,"13":25})
    );
}
