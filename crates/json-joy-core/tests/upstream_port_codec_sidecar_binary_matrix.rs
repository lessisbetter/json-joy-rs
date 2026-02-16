use json_joy_core::codec_sidecar_binary::{
    decode_sidecar_to_model_binary, encode_model_binary_to_sidecar,
};
use json_joy_core::less_db_compat::{create_model, model_to_binary};

#[test]
fn upstream_port_codec_sidecar_binary_matrix_roundtrip_from_runtime_models() {
    let cases = [
        (83101_u64, serde_json::json!({"a": 1, "b": "x"})),
        (83102_u64, serde_json::json!({"arr": [1, 2, 3]})),
        (
            83103_u64,
            serde_json::json!({"meta": {"active": true, "score": 2}}),
        ),
        (
            83104_u64,
            serde_json::json!({"root": [1, {"k": "v"}, false]}),
        ),
    ];

    for (sid, value) in cases {
        let model = create_model(&value, sid).expect("model create");
        let binary = model_to_binary(&model);
        let (view, meta) = encode_model_binary_to_sidecar(&binary).expect("sidecar encode");
        let roundtrip = decode_sidecar_to_model_binary(&view, &meta).expect("sidecar decode");
        assert_eq!(binary, roundtrip, "sidecar roundtrip mismatch sid={sid}");
    }
}
