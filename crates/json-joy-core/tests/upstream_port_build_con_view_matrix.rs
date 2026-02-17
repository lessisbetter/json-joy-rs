use json_joy_core::diff_runtime::diff_model_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::patch::{ConValue, DecodedOp, Patch};

#[test]
fn upstream_port_build_con_view_object_scalar_insert_uses_new_con_json() {
    let sid = 97100;
    let model = create_model(&serde_json::json!({}), sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);
    let next = serde_json::json!({"a": 1});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("patch decode must succeed");

    assert!(
        decoded.decoded_ops().iter().any(|op| {
            matches!(
                op,
                DecodedOp::NewCon {
                    value: ConValue::Json(v),
                    ..
                } if *v == serde_json::json!(1)
            )
        }),
        "expected NewCon(Json(1)) for object scalar insert"
    );
}

#[test]
fn upstream_port_build_con_view_array_scalar_insert_wraps_with_val_node() {
    let sid = 97101;
    let model = create_model(&serde_json::json!({"a": []}), sid).expect("create_model must succeed");
    let base_model = model_to_binary(&model);
    let next = serde_json::json!({"a": [1]});

    let patch = diff_model_to_patch_bytes(&base_model, &next, sid)
        .expect("diff should succeed")
        .expect("non-noop diff expected");
    let decoded = Patch::from_binary(&patch).expect("patch decode must succeed");

    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::NewVal { .. })),
        "expected NewVal wrapper for array scalar insertion"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsVal { .. })),
        "expected InsVal wiring from wrapper ValNode to scalar ConNode"
    );
    assert!(
        decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, DecodedOp::InsArr { .. })),
        "expected InsArr to attach wrapped scalar to array"
    );
}
