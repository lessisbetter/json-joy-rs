use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;

// Upstream references:
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/PatchBuilder.ts
// - /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/codec/binary/Encoder.ts

#[test]
fn upstream_port_patch_builder_object_set_matches_expected_binary() {
    let sid = 78001;
    let time = 3;
    let ops = vec![
        DecodedOp::NewCon {
            id: Timestamp { sid, time: 3 },
            value: ConValue::Json(serde_json::json!(1)),
        },
        DecodedOp::InsObj {
            id: Timestamp { sid, time: 4 },
            obj: Timestamp { sid, time: 1 },
            data: vec![("a".to_string(), Timestamp { sid, time: 3 })],
        },
    ];

    let encoded = encode_patch_from_ops(sid, time, &ops).expect("encode must succeed");
    // Expected bytes come from pinned oracle fixture:
    // lessdb_model_manager_01_create_diff_apply_v1
    let expected = decode_hex("b1e10403f70200015101616103");
    assert_eq!(encoded, expected);

    let patch = Patch::from_binary(&encoded).expect("encoded patch should decode");
    assert_eq!(patch.opcodes(), &[0, 10]);
    assert_eq!(patch.op_count(), 2);
    assert_eq!(patch.span(), 2);
    assert_eq!(patch.id(), Some((sid, time)));
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

#[test]
fn upstream_port_patch_builder_rejects_non_canonical_timeline() {
    let sid = 90001;
    let time = 5;
    let ops = vec![
        DecodedOp::NewObj {
            id: Timestamp { sid, time: 5 },
        },
        // must be time=6, deliberately invalid to prove timeline validation.
        DecodedOp::NewCon {
            id: Timestamp { sid, time: 8 },
            value: ConValue::Json(serde_json::json!(1)),
        },
    ];

    let err = encode_patch_from_ops(sid, time, &ops).expect_err("must fail");
    let msg = err.to_string();
    assert!(msg.contains("operation id must match patch timeline"));
}
