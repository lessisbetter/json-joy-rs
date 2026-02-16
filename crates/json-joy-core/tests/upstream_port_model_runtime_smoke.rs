use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream references:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/Model.ts
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/nodes/obj/ObjNode.ts
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/nodes/val/ValNode.ts

#[test]
fn upstream_port_runtime_apply_object_insert_from_empty_object_root() {
    // model_roundtrip_empty_object_v1 fixture payload (sid=73012).
    let base_model = decode_hex("00000002114001b4ba0402");
    let sid = 73012;

    let ops = vec![
        DecodedOp::NewCon {
            id: Timestamp { sid, time: 2 },
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 3 },
            obj: Timestamp { sid, time: 1 },
            data: vec![("x".to_string(), Timestamp { sid, time: 2 })],
        },
    ];

    let patch_bytes = encode_patch_from_ops(sid, 2, &ops).expect("encode must succeed");
    let patch = Patch::from_binary(&patch_bytes).expect("patch must decode");

    let mut runtime =
        RuntimeModel::from_model_binary(&base_model).expect("base model decode must succeed");
    runtime
        .apply_patch(&patch)
        .expect("runtime apply must succeed");

    assert_eq!(runtime.view_json(), serde_json::json!({"x": 1}));
}

#[test]
fn upstream_port_runtime_duplicate_apply_is_stable_for_obj_insert() {
    let base_model = decode_hex("00000002114001b4ba0402");
    let sid = 73012;

    let ops = vec![
        DecodedOp::NewCon {
            id: Timestamp { sid, time: 2 },
            value: ConValue::Json(serde_json::json!("A")),
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 3 },
            obj: Timestamp { sid, time: 1 },
            data: vec![("title".to_string(), Timestamp { sid, time: 2 })],
        },
    ];
    let patch_bytes = encode_patch_from_ops(sid, 2, &ops).expect("encode must succeed");
    let patch = Patch::from_binary(&patch_bytes).expect("patch must decode");

    let mut runtime =
        RuntimeModel::from_model_binary(&base_model).expect("base model decode must succeed");
    runtime
        .apply_patch(&patch)
        .expect("first apply must succeed");
    runtime
        .apply_patch(&patch)
        .expect("second apply must succeed");

    assert_eq!(runtime.view_json(), serde_json::json!({"title": "A"}));
}

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex string must have even length"
    );
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}
