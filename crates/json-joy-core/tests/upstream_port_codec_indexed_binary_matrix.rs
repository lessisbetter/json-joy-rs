use std::collections::BTreeMap;

use json_joy_core::codec_indexed_binary::{
    decode_fields_to_model_binary, encode_model_binary_to_fields,
};
use json_joy_core::less_db_compat::{create_model, model_to_binary};

#[test]
fn upstream_port_codec_indexed_binary_matrix_roundtrip_from_runtime_models() {
    let cases = [
        (83001_u64, serde_json::json!({"a": 1, "b": "x"})),
        (
            83002_u64,
            serde_json::json!({"doc": {"title": "t", "tags": ["x", "y"]}}),
        ),
        (83003_u64, serde_json::json!([1, 2, {"k": true}])),
        (
            83004_u64,
            serde_json::json!({"deep": {"a": {"b": {"c": 1}}}}),
        ),
    ];

    for (sid, value) in cases {
        let model = create_model(&value, sid).expect("model create");
        let binary = model_to_binary(&model);
        let fields: BTreeMap<String, Vec<u8>> =
            encode_model_binary_to_fields(&binary).expect("indexed encode");
        let roundtrip = decode_fields_to_model_binary(&fields).expect("indexed decode");
        assert_eq!(binary, roundtrip, "indexed roundtrip mismatch sid={sid}");
    }
}
