use std::fs;
use std::path::{Path, PathBuf};

use json_joy_core::patch::{ConValue, DecodedOp, Patch, Timespan, Timestamp};
use json_joy_core::patch_builder::encode_patch_from_ops;
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("compat")
        .join("fixtures")
}

fn read_json(path: &Path) -> Value {
    let data = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", path))
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

fn as_u64(v: &Value, label: &str) -> u64 {
    v.as_u64().unwrap_or_else(|| panic!("{label} must be u64"))
}

fn as_str<'a>(v: &'a Value, label: &str) -> &'a str {
    v.as_str().unwrap_or_else(|| panic!("{label} must be string"))
}

fn as_ts(v: &Value, label: &str) -> Timestamp {
    let arr = v
        .as_array()
        .unwrap_or_else(|| panic!("{label} must be [sid,time]"));
    assert_eq!(arr.len(), 2, "{label} must have two elements");
    Timestamp {
        sid: as_u64(&arr[0], "sid"),
        time: as_u64(&arr[1], "time"),
    }
}

fn canonical_ops(input: &Value) -> Vec<DecodedOp> {
    let sid = input["sid"].as_u64().expect("input.sid must be u64");
    let mut op_time = input["time"].as_u64().expect("input.time must be u64");
    let ops = input["ops"].as_array().expect("input.ops must be array");

    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let id = Timestamp {
            sid,
            time: op_time,
        };
        let parsed = match as_str(&op["op"], "op.op") {
            "new_con" => DecodedOp::NewCon {
                id,
                value: ConValue::Json(op["value"].clone()),
            },
            "new_con_ref" => DecodedOp::NewCon {
                id,
                value: ConValue::Ref(as_ts(&op["value_ref"], "op.value_ref")),
            },
            "new_val" => DecodedOp::NewVal { id },
            "new_obj" => DecodedOp::NewObj { id },
            "new_vec" => DecodedOp::NewVec { id },
            "new_str" => DecodedOp::NewStr { id },
            "new_bin" => DecodedOp::NewBin { id },
            "new_arr" => DecodedOp::NewArr { id },
            "ins_val" => DecodedOp::InsVal {
                id,
                obj: as_ts(&op["obj"], "op.obj"),
                val: as_ts(&op["val"], "op.val"),
            },
            "ins_obj" => {
                let tuples = op["data"].as_array().expect("op.data must be array");
                let mut data = Vec::with_capacity(tuples.len());
                for tuple in tuples {
                    let t = tuple.as_array().expect("tuple must be [key,id]");
                    assert_eq!(t.len(), 2, "tuple must have 2 elements");
                    data.push((
                        as_str(&t[0], "tuple key").to_string(),
                        as_ts(&t[1], "tuple id"),
                    ));
                }
                DecodedOp::InsObj {
                    id,
                    obj: as_ts(&op["obj"], "op.obj"),
                    data,
                }
            }
            "ins_vec" => {
                let tuples = op["data"].as_array().expect("op.data must be array");
                let mut data = Vec::with_capacity(tuples.len());
                for tuple in tuples {
                    let t = tuple.as_array().expect("tuple must be [idx,id]");
                    assert_eq!(t.len(), 2, "tuple must have 2 elements");
                    data.push((as_u64(&t[0], "tuple idx"), as_ts(&t[1], "tuple id")));
                }
                DecodedOp::InsVec {
                    id,
                    obj: as_ts(&op["obj"], "op.obj"),
                    data,
                }
            }
            "ins_str" => DecodedOp::InsStr {
                id,
                obj: as_ts(&op["obj"], "op.obj"),
                reference: as_ts(&op["ref"], "op.ref"),
                data: as_str(&op["data"], "op.data").to_string(),
            },
            "ins_bin" => {
                let bytes = op["data"]
                    .as_array()
                    .expect("op.data must be byte array")
                    .iter()
                    .map(|v| u8::try_from(v.as_u64().expect("byte must be u64")).expect("byte out of range"))
                    .collect();
                DecodedOp::InsBin {
                    id,
                    obj: as_ts(&op["obj"], "op.obj"),
                    reference: as_ts(&op["ref"], "op.ref"),
                    data: bytes,
                }
            }
            "ins_arr" => {
                let ids = op["data"].as_array().expect("op.data must be id array");
                DecodedOp::InsArr {
                    id,
                    obj: as_ts(&op["obj"], "op.obj"),
                    reference: as_ts(&op["ref"], "op.ref"),
                    data: ids.iter().map(|v| as_ts(v, "ins_arr id")).collect(),
                }
            }
            "upd_arr" => DecodedOp::UpdArr {
                id,
                obj: as_ts(&op["obj"], "op.obj"),
                reference: as_ts(&op["ref"], "op.ref"),
                val: as_ts(&op["val"], "op.val"),
            },
            "del" => {
                let spans = op["what"].as_array().expect("op.what must be array");
                let mut what = Vec::with_capacity(spans.len());
                for span in spans {
                    let t = span.as_array().expect("timespan must be [sid,time,span]");
                    assert_eq!(t.len(), 3, "timespan must have 3 elements");
                    what.push(Timespan {
                        sid: as_u64(&t[0], "timespan sid"),
                        time: as_u64(&t[1], "timespan time"),
                        span: as_u64(&t[2], "timespan span"),
                    });
                }
                DecodedOp::Del {
                    id,
                    obj: as_ts(&op["obj"], "op.obj"),
                    what,
                }
            }
            "nop" => DecodedOp::Nop {
                id,
                len: as_u64(&op["len"], "op.len"),
            },
            other => panic!("unsupported canonical op: {other}"),
        };

        op_time += parsed.span();
        out.push(parsed);
    }

    out
}

#[test]
fn patch_canonical_encode_fixtures_match_oracle_binary() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("patch_canonical_encode") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let input = &fixture["input"];
        let sid = input["sid"].as_u64().expect("input.sid must be u64");
        let time = input["time"].as_u64().expect("input.time must be u64");
        let ops = canonical_ops(input);

        let encoded = encode_patch_from_ops(sid, time, &ops)
            .unwrap_or_else(|e| panic!("canonical encode failed for {}: {e}", entry["name"]));
        let expected = decode_hex(
            fixture["expected"]["patch_binary_hex"]
                .as_str()
                .expect("expected.patch_binary_hex must be string"),
        );

        assert_eq!(
            encoded, expected,
            "canonical encode byte mismatch for fixture {}",
            entry["name"]
        );

        let patch = Patch::from_binary(&encoded)
            .unwrap_or_else(|e| panic!("encoded canonical patch failed to decode for {}: {e}", entry["name"]));
        let expected_op_count = fixture["expected"]["patch_op_count"]
            .as_u64()
            .expect("expected.patch_op_count must be u64");
        let expected_span = fixture["expected"]["patch_span"]
            .as_u64()
            .expect("expected.patch_span must be u64");
        let expected_opcodes: Vec<u8> = fixture["expected"]["patch_opcodes"]
            .as_array()
            .expect("expected.patch_opcodes must be array")
            .iter()
            .map(|v| {
                let n = v
                    .as_u64()
                    .or_else(|| v.as_i64().and_then(|i| u64::try_from(i).ok()))
                    .unwrap_or_else(|| panic!("opcode must be integer, got {v:?}"));
                u8::try_from(n).expect("opcode must fit u8")
            })
            .collect();

        assert_eq!(patch.op_count(), expected_op_count);
        assert_eq!(patch.span(), expected_span);
        assert_eq!(patch.opcodes(), expected_opcodes.as_slice());
    }

    assert!(seen >= 4, "expected at least 4 patch_canonical_encode fixtures");
}
