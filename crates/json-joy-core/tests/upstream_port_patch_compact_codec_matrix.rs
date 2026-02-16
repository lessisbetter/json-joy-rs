use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timespan, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use json_joy_core::patch_compact_codec::{decode_patch_compact, encode_patch_compact};
use serde_json::json;

fn mk_patch(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode");
    Patch::from_binary(&bytes).expect("decode")
}

#[test]
fn upstream_port_patch_compact_codec_encode_shape_matrix() {
    let sid = 333;
    let patch = mk_patch(
        sid,
        10,
        &[
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 10 },
                value: ConValue::Json(json!({"a":1})),
            },
            DecodedOp::NewStr {
                id: Timestamp { sid, time: 11 },
            },
            DecodedOp::InsStr {
                id: Timestamp { sid, time: 12 },
                obj: Timestamp { sid, time: 11 },
                reference: Timestamp { sid, time: 11 },
                data: "ab".to_string(),
            },
            DecodedOp::InsBin {
                id: Timestamp { sid, time: 14 },
                obj: Timestamp { sid, time: 11 },
                reference: Timestamp { sid, time: 12 },
                data: vec![1, 2, 3],
            },
            DecodedOp::Del {
                id: Timestamp { sid, time: 17 },
                obj: Timestamp { sid, time: 11 },
                what: vec![Timespan {
                    sid,
                    time: 12,
                    span: 2,
                }],
            },
        ],
    );
    let compact = encode_patch_compact(&patch).expect("compact encode");
    let rows = compact.as_array().expect("array");
    assert_eq!(rows[0], json!([[333, 10]]));
    assert_eq!(rows[1], json!([0, {"a":1}]));
    assert_eq!(rows[2], json!([4]));
    assert_eq!(rows[3], json!([12, 11, 11, "ab"]));
    assert_eq!(rows[4], json!([13, 11, 12, "AQID"]));
    assert_eq!(rows[5], json!([16, 11, [[12, 2]]]));
}

#[test]
fn upstream_port_patch_compact_codec_roundtrip_to_binary_matrix() {
    let sid = 777;
    let patch = mk_patch(
        sid,
        100,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 100 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 101 },
                value: ConValue::Json(json!(5)),
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 102 },
                obj: Timestamp { sid, time: 100 },
                data: vec![("k".to_string(), Timestamp { sid, time: 101 })],
            },
            DecodedOp::Nop {
                id: Timestamp { sid, time: 103 },
                len: 3,
            },
        ],
    );
    let compact = encode_patch_compact(&patch).expect("compact encode");
    let decoded = decode_patch_compact(&compact).expect("compact decode");
    assert_eq!(decoded.to_binary(), patch.to_binary());
}

#[test]
fn upstream_port_patch_compact_codec_decodes_server_header() {
    let compact = json!([[42], [0, 1], [17, 2]]);
    let patch = decode_patch_compact(&compact).expect("decode");
    assert_eq!(patch.id(), Some((1, 42)));
    assert_eq!(patch.op_count(), 2);
    assert_eq!(patch.span(), 3);
}
