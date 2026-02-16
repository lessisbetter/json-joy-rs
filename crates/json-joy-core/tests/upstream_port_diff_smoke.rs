use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream references:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/Model.ts (api.diff/applyPatch)

#[test]
fn upstream_port_diff_noop_on_equal_object_returns_none() {
    // model_roundtrip_empty_object_v1 fixture payload (sid=73012).
    let base_model = decode_hex("00000002114001b4ba0402");

    let patch = diff_model_to_patch_bytes(&base_model, &serde_json::json!({}), 73012)
        .expect("diff should succeed");
    assert!(patch.is_none(), "equal object diff must be None");
}

#[test]
fn upstream_port_diff_apply_reaches_target_view() {
    // model_roundtrip_empty_object_v1 fixture payload (sid=73012).
    let base_model = decode_hex("00000002114001b4ba0402");
    let next = serde_json::json!({"a": 1, "b": "x"});

    let patch = diff_model_to_patch_bytes(&base_model, &next, 73012)
        .expect("diff should succeed")
        .expect("non-noop diff expected");

    let mut runtime = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    runtime.apply_patch(&decoded).expect("runtime apply must succeed");

    assert_eq!(runtime.view_json(), next);
}

#[test]
fn upstream_port_diff_nested_object_delta_reaches_target_view() {
    // Ports upstream JsonCrdtDiff.diffObj behavior for nested object edits:
    // source-key delete writes first, then destination-key inserts/updates.
    let sid = 88001;
    let initial = serde_json::json!({"doc": {"a": 1, "b": "x", "c": true}});
    let next = serde_json::json!({"doc": {"a": 2, "b": "x", "d": null}});
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");

    let mut runtime = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    runtime.apply_patch(&decoded).expect("runtime apply must succeed");
    assert_eq!(runtime.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_array_key_delta_reaches_target_view() {
    // Ports multiple ArrNode edits in one root object diff pass.
    let sid = 88002;
    let initial = serde_json::json!({
        "a": [1, "x", 2],
        "b": ["q", 9]
    });
    let next = serde_json::json!({
        "a": [1, "x", "y", 2],
        "b": [9]
    });
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");

    let mut runtime = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    runtime.apply_patch(&decoded).expect("runtime apply must succeed");
    assert_eq!(runtime.view_json(), next);
}

#[test]
fn upstream_port_diff_vec_index_updates_use_ins_vec() {
    // Ports upstream JsonCrdtDiff.diffVec behavior for index updates/deletes.
    let sid = 88003;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let vec_id = Timestamp { sid, time: 3 };
    let one = Timestamp { sid, time: 4 };
    let two = Timestamp { sid, time: 5 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewVec { id: vec_id },
        DecodedOp::NewCon {
            id: one,
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::NewCon {
            id: two,
            value: ConValue::Json(serde_json::json!(2)),
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 6 },
            obj: vec_id,
            data: vec![(0, one), (1, two)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 7 },
            obj: root,
            data: vec![("v".to_string(), vec_id)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"v": [1, 3]});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsVec { .. })),
        "diff patch must contain ins_vec"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_bin_delta_uses_ins_bin_and_del() {
    // Ports upstream JsonCrdtDiff.diffBin behavior.
    let sid = 88004;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let bin = Timestamp { sid, time: 3 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewBin { id: bin },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 4 },
            obj: bin,
            reference: bin,
            data: vec![1, 2, 3],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 7 },
            obj: root,
            data: vec![("b".to_string(), bin)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"b": {"0": 1, "1": 4, "2": 3, "3": 5}});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsBin { .. })),
        "diff patch must contain ins_bin"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::Del { .. })),
        "diff patch must contain del"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_nested_vec_delta_uses_ins_vec() {
    let sid = 88005;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let doc = Timestamp { sid, time: 3 };
    let vec_id = Timestamp { sid, time: 4 };
    let one = Timestamp { sid, time: 5 };
    let two = Timestamp { sid, time: 6 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewObj { id: doc },
        DecodedOp::NewVec { id: vec_id },
        DecodedOp::NewCon {
            id: one,
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::NewCon {
            id: two,
            value: ConValue::Json(serde_json::json!(2)),
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 7 },
            obj: vec_id,
            data: vec![(0, one), (1, two)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 8 },
            obj: doc,
            data: vec![("v".to_string(), vec_id)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 9 },
            obj: root,
            data: vec![("doc".to_string(), doc)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"doc": {"v": [1, 3]}});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsVec { .. })),
        "nested diff patch must contain ins_vec"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_nested_bin_delta_uses_ins_bin() {
    let sid = 88006;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let doc = Timestamp { sid, time: 3 };
    let bin = Timestamp { sid, time: 4 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewObj { id: doc },
        DecodedOp::NewBin { id: bin },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 5 },
            obj: bin,
            reference: bin,
            data: vec![1, 2, 3],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 8 },
            obj: doc,
            data: vec![("b".to_string(), bin)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 9 },
            obj: root,
            data: vec![("doc".to_string(), doc)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"doc": {"b": {"0": 1, "1": 4, "2": 3}}});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsBin { .. })),
        "nested diff patch must contain ins_bin"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_nested_arr_delta_uses_ins_arr_and_del() {
    let sid = 88007;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let doc = Timestamp { sid, time: 3 };
    let arr = Timestamp { sid, time: 4 };
    let one = Timestamp { sid, time: 5 };
    let two = Timestamp { sid, time: 6 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewObj { id: doc },
        DecodedOp::NewArr { id: arr },
        DecodedOp::NewCon {
            id: one,
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::NewCon {
            id: two,
            value: ConValue::Json(serde_json::json!(2)),
        },
        DecodedOp::InsArr {
            id: Timestamp { sid, time: 7 },
            obj: arr,
            reference: arr,
            data: vec![one, two],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 9 },
            obj: doc,
            data: vec![("a".to_string(), arr)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 10 },
            obj: root,
            data: vec![("doc".to_string(), doc)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"doc": {"a": [1, 3]}});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsArr { .. })),
        "nested diff patch must contain ins_arr"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::Del { .. })),
        "nested diff patch must contain del"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_string_deltas_use_ins_str() {
    let sid = 88008;
    let initial = serde_json::json!({"a": "hello", "b": "world"});
    let next = serde_json::json!({"a": "hallo", "b": "word"});
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let str_insert_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsStr { .. }))
        .count();
    let str_mut_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsStr { .. } | DecodedOp::Del { .. }))
        .count();
    assert!(str_insert_ops >= 1, "expected at least one ins_str op");
    assert!(str_mut_ops >= 2, "expected >=2 string mutation ops across keys");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_bin_deltas_use_ins_bin() {
    let sid = 88009;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let b1 = Timestamp { sid, time: 3 };
    let b2 = Timestamp { sid, time: 7 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewBin { id: b1 },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 4 },
            obj: b1,
            reference: b1,
            data: vec![1, 2, 3],
        },
        DecodedOp::NewBin {
            id: Timestamp { sid, time: 7 },
        },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 8 },
            obj: b2,
            reference: b2,
            data: vec![4, 5, 6],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 11 },
            obj: root,
            data: vec![("x".to_string(), b1), ("y".to_string(), b2)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({
        "x": {"0": 1, "1": 9, "2": 3},
        "y": {"0": 4, "1": 8, "2": 6}
    });

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let bin_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsBin { .. }))
        .count();
    assert!(bin_ops >= 2, "expected >=2 ins_bin ops for multi-key bin delta");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_vec_deltas_use_ins_vec() {
    let sid = 88010;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let v1 = Timestamp { sid, time: 3 };
    let v2 = Timestamp { sid, time: 4 };
    let c1 = Timestamp { sid, time: 5 };
    let c2 = Timestamp { sid, time: 6 };
    let c3 = Timestamp { sid, time: 7 };
    let c4 = Timestamp { sid, time: 8 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewVec { id: v1 },
        DecodedOp::NewVec { id: v2 },
        DecodedOp::NewCon {
            id: c1,
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::NewCon {
            id: c2,
            value: ConValue::Json(serde_json::json!(2)),
        },
        DecodedOp::NewCon {
            id: c3,
            value: ConValue::Json(serde_json::json!(3)),
        },
        DecodedOp::NewCon {
            id: c4,
            value: ConValue::Json(serde_json::json!(4)),
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 9 },
            obj: v1,
            data: vec![(0, c1), (1, c2)],
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 10 },
            obj: v2,
            data: vec![(0, c3), (1, c4)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 11 },
            obj: root,
            data: vec![("x".to_string(), v1), ("y".to_string(), v2)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"x": [1, 9], "y": [3, 8]});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let vec_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsVec { .. }))
        .count();
    assert!(vec_ops >= 2, "expected >=2 ins_vec ops for multi-key vec delta");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_nested_obj_deltas_use_ins_obj() {
    let sid = 88011;
    let initial = serde_json::json!({
        "a": {"x": 1, "y": true},
        "b": {"k": "v", "m": null}
    });
    let next = serde_json::json!({
        "a": {"x": 2, "z": false},
        "b": {"k": "vv", "m": null, "n": 9}
    });
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let obj_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsObj { .. }))
        .count();
    assert!(obj_ops >= 2, "expected >=2 ins_obj ops for multi-root nested object delta");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_nested_string_deltas_use_ins_str() {
    let sid = 88012;
    let initial = serde_json::json!({
        "left": {"t": "alpha"},
        "right": {"t": "omega"}
    });
    let next = serde_json::json!({
        "left": {"t": "alphi"},
        "right": {"t": "omeg"}
    });
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let str_insert_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsStr { .. }))
        .count();
    let str_mut_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsStr { .. } | DecodedOp::Del { .. }))
        .count();
    assert!(str_insert_ops >= 1, "expected at least one ins_str op");
    assert!(str_mut_ops >= 2, "expected >=2 string mutation ops across nested roots");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_nested_bin_deltas_use_ins_bin() {
    let sid = 88013;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let left = Timestamp { sid, time: 3 };
    let right = Timestamp { sid, time: 4 };
    let lb = Timestamp { sid, time: 5 };
    let rb = Timestamp { sid, time: 9 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewObj { id: left },
        DecodedOp::NewObj { id: right },
        DecodedOp::NewBin { id: lb },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 6 },
            obj: lb,
            reference: lb,
            data: vec![1, 2, 3],
        },
        DecodedOp::NewBin {
            id: Timestamp { sid, time: 9 },
        },
        DecodedOp::InsBin {
            id: Timestamp { sid, time: 10 },
            obj: rb,
            reference: rb,
            data: vec![4, 5, 6],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 13 },
            obj: left,
            data: vec![("b".to_string(), lb)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 14 },
            obj: right,
            data: vec![("b".to_string(), rb)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 15 },
            obj: root,
            data: vec![("left".to_string(), left), ("right".to_string(), right)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({
        "left": {"b": {"0": 1, "1": 9, "2": 3}},
        "right": {"b": {"0": 4, "1": 8, "2": 6}}
    });

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let bin_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsBin { .. }))
        .count();
    assert!(bin_ops >= 2, "expected >=2 ins_bin ops for multi-root nested bin deltas");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_nested_arr_deltas_use_ins_arr() {
    let sid = 88014;
    let initial = serde_json::json!({
        "left": {"a": [1, 2]},
        "right": {"a": [3, 4]}
    });
    let next = serde_json::json!({
        "left": {"a": [1, 9]},
        "right": {"a": [3, 8]}
    });
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let arr_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsArr { .. }))
        .count();
    assert!(arr_ops >= 2, "expected >=2 ins_arr ops for multi-root nested array deltas");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_multi_root_nested_vec_deltas_use_ins_vec() {
    let sid = 88015;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let left = Timestamp { sid, time: 3 };
    let right = Timestamp { sid, time: 4 };
    let lv = Timestamp { sid, time: 5 };
    let rv = Timestamp { sid, time: 6 };
    let c1 = Timestamp { sid, time: 7 };
    let c2 = Timestamp { sid, time: 8 };
    let c3 = Timestamp { sid, time: 9 };
    let c4 = Timestamp { sid, time: 10 };
    let ops = vec![
        DecodedOp::NewObj { id: root },
        DecodedOp::InsVal {
            id: Timestamp { sid, time: 2 },
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        },
        DecodedOp::NewObj { id: left },
        DecodedOp::NewObj { id: right },
        DecodedOp::NewVec { id: lv },
        DecodedOp::NewVec { id: rv },
        DecodedOp::NewCon {
            id: c1,
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::NewCon {
            id: c2,
            value: ConValue::Json(serde_json::json!(2)),
        },
        DecodedOp::NewCon {
            id: c3,
            value: ConValue::Json(serde_json::json!(3)),
        },
        DecodedOp::NewCon {
            id: c4,
            value: ConValue::Json(serde_json::json!(4)),
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 11 },
            obj: lv,
            data: vec![(0, c1), (1, c2)],
        },
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 12 },
            obj: rv,
            data: vec![(0, c3), (1, c4)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 13 },
            obj: left,
            data: vec![("v".to_string(), lv)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 14 },
            obj: right,
            data: vec![("v".to_string(), rv)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 15 },
            obj: root,
            data: vec![("left".to_string(), left), ("right".to_string(), right)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime.apply_patch(&seed_patch).expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({
        "left": {"v": [1, 9]},
        "right": {"v": [3, 8]}
    });

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    let vec_ops = decoded
        .decoded_ops()
        .iter()
        .filter(|op| matches!(op, DecodedOp::InsVec { .. }))
        .count();
    assert!(vec_ops >= 2, "expected >=2 ins_vec ops for multi-root nested vec deltas");

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_mixed_root_recursive_emits_child_and_root_ops() {
    let sid = 88016;
    let initial = serde_json::json!({
        "s": "hello",
        "arr": [1, 2],
        "obj": {"a": 1},
        "drop": true
    });
    let next = serde_json::json!({
        "s": "hallo",
        "arr": [1, 9],
        "obj": {"a": 2, "b": null},
        "add": 42
    });
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsStr { .. })),
        "expected child ins_str op"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsArr { .. })),
        "expected child ins_arr op"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsObj { .. })),
        "expected ins_obj op"
    );

    let mut applied = RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex string must have even length");
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}
