use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timespan, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use json_joy_core::patch_verbose_codec::{decode_patch_verbose, encode_patch_verbose};
use serde_json::json;

fn mk_patch(sid: u64, time: u64, ops: &[DecodedOp]) -> Patch {
    let bytes = encode_patch_from_ops(sid, time, ops).expect("encode");
    Patch::from_binary(&bytes).expect("decode")
}

#[test]
fn upstream_port_patch_verbose_codec_encode_shape_matrix() {
    let sid = 1234;
    let patch = mk_patch(
        sid,
        5,
        &[
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 5 },
                value: ConValue::Json(json!("x")),
            },
            DecodedOp::NewBin {
                id: Timestamp { sid, time: 6 },
            },
            DecodedOp::InsBin {
                id: Timestamp { sid, time: 7 },
                obj: Timestamp { sid, time: 6 },
                reference: Timestamp { sid, time: 6 },
                data: vec![255],
            },
            DecodedOp::Del {
                id: Timestamp { sid, time: 8 },
                obj: Timestamp { sid, time: 6 },
                what: vec![Timespan {
                    sid,
                    time: 7,
                    span: 1,
                }],
            },
        ],
    );
    let verbose = encode_patch_verbose(&patch).expect("encode");
    assert_eq!(verbose["id"], json!([1234, 5]));
    assert_eq!(verbose["ops"][0], json!({"op":"new_con","value":"x"}));
    assert_eq!(verbose["ops"][1], json!({"op":"new_bin"}));
    assert_eq!(
        verbose["ops"][2],
        json!({"op":"ins_bin","obj":[1234,6],"after":[1234,6],"value":"/w=="})
    );
    assert_eq!(
        verbose["ops"][3],
        json!({"op":"del","obj":[1234,6],"what":[[1234,7,1]]})
    );
}

#[test]
fn upstream_port_patch_verbose_codec_roundtrip_to_binary_matrix() {
    let sid = 9090;
    let patch = mk_patch(
        sid,
        20,
        &[
            DecodedOp::NewObj {
                id: Timestamp { sid, time: 20 },
            },
            DecodedOp::NewCon {
                id: Timestamp { sid, time: 21 },
                value: ConValue::Json(json!(true)),
            },
            DecodedOp::InsObj {
                id: Timestamp { sid, time: 22 },
                obj: Timestamp { sid, time: 20 },
                data: vec![("ok".to_string(), Timestamp { sid, time: 21 })],
            },
            DecodedOp::Nop {
                id: Timestamp { sid, time: 23 },
                len: 2,
            },
        ],
    );
    let verbose = encode_patch_verbose(&patch).expect("encode verbose");
    let decoded = decode_patch_verbose(&verbose).expect("decode verbose");
    assert_eq!(decoded.to_binary(), patch.to_binary());
}

#[test]
fn upstream_port_patch_verbose_codec_decode_accepts_number_timestamps() {
    let verbose = json!({
      "id": [1, 42],
      "ops": [
        {"op":"new_con","value":1},
        {"op":"nop","len":2}
      ]
    });
    let decoded = decode_patch_verbose(&verbose).expect("decode");
    assert_eq!(decoded.id(), Some((1, 42)));
    assert_eq!(decoded.span(), 3);
}
