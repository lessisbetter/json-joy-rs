use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream references:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/__tests__/JsonCrdtDiff.spec.ts

#[test]
fn upstream_port_diff_any_matrix_root_type_mismatch_replaces_origin_value() {
    let sid = 88901;
    let model = create_model(&serde_json::json!("abc"), sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);
    let next = serde_json::json!([1, 2, 3]);

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsVal { obj, .. } if *obj == Timestamp { sid: 0, time: 0 })),
        "root type mismatch must write ORIGIN through ins_val"
    );
    assert!(
        !decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsStr { .. })),
        "root type mismatch should not try in-place str edits"
    );

    let mut applied =
        RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_any_matrix_object_field_type_mismatch_replaces_field_value() {
    let sid = 88902;
    let initial = serde_json::json!({"k": "abc"});
    let next = serde_json::json!({"k": 123});
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
            .any(|op| matches!(op, DecodedOp::InsObj { data, .. } if data.iter().any(|(k, _)| k == "k"))),
        "object field type mismatch should use ins_obj replacement path"
    );
    assert!(
        !decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsStr { .. })),
        "field type mismatch should not emit in-place string edits"
    );

    let mut applied =
        RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

#[test]
fn upstream_port_diff_any_matrix_vec_index_type_mismatch_uses_ins_vec() {
    let sid = 88903;
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let root = Timestamp { sid, time: 1 };
    let vec_id = Timestamp { sid, time: 3 };
    let one = Timestamp { sid, time: 4 };
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
        DecodedOp::InsVec {
            id: Timestamp { sid, time: 5 },
            obj: vec_id,
            data: vec![(0, one)],
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 6 },
            obj: root,
            data: vec![("v".to_string(), vec_id)],
        },
    ];
    let seed = encode_patch_from_ops(sid, 1, &ops).expect("seed patch encode must succeed");
    let seed_patch = Patch::from_binary(&seed).expect("seed patch decode must succeed");
    runtime
        .apply_patch(&seed_patch)
        .expect("seed apply must succeed");
    let base_model = runtime
        .to_model_binary_like()
        .expect("runtime model encode must succeed");
    let next = serde_json::json!({"v": ["xyz"]});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("generated patch must decode");
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsVec { .. })),
        "vec index type mismatch should update slot using ins_vec"
    );

    let mut applied =
        RuntimeModel::from_model_binary(&base_model).expect("runtime decode must succeed");
    applied
        .apply_patch(&decoded)
        .expect("runtime apply must succeed");
    assert_eq!(applied.view_json(), next);
}

