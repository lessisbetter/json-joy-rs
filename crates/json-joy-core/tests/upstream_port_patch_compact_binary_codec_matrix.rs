use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use json_joy_core::patch_compact_binary_codec::{
    decode_patch_compact_binary, encode_patch_compact_binary,
};

fn mk_patch(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode");
    Patch::from_binary(&bytes).expect("decode")
}

#[test]
fn upstream_port_patch_compact_binary_codec_roundtrip_to_binary_matrix() {
    let sid = 5001;
    let patch = mk_patch(
        sid,
        10,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 10 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 11 },
                value: ConValue::Json(serde_json::json!(123)),
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 12 },
                obj: Timestamp { sid, time: 10 },
                data: vec![("n".to_string(), Timestamp { sid, time: 11 })],
            },
        ],
    );
    let encoded = encode_patch_compact_binary(&patch).expect("compact-binary encode");
    assert!(!encoded.is_empty(), "compact-binary must not be empty");
    let decoded = decode_patch_compact_binary(&encoded).expect("compact-binary decode");
    assert_eq!(decoded.to_binary(), patch.to_binary());
}

#[test]
fn upstream_port_patch_compact_binary_codec_rejects_invalid_cbor() {
    let err = decode_patch_compact_binary(&[0xff, 0xff]).expect_err("must reject invalid cbor");
    assert_eq!(err.to_string(), "invalid compact-binary cbor payload");
}

