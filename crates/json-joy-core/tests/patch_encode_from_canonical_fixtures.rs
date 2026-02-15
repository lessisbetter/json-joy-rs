use std::fs;
use std::path::{Path, PathBuf};

use ciborium::value::{Integer, Value as CborValue};
use json_joy_core::patch::Patch;
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

fn as_ts(v: &Value, label: &str) -> (u64, u64) {
    let arr = v
        .as_array()
        .unwrap_or_else(|| panic!("{label} must be [sid,time]"));
    assert_eq!(arr.len(), 2, "{label} must have two elements");
    (as_u64(&arr[0], "sid"), as_u64(&arr[1], "time"))
}

fn json_to_cbor(v: &Value) -> CborValue {
    match v {
        Value::Null => CborValue::Null,
        Value::Bool(b) => CborValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CborValue::Integer(Integer::from(i))
            } else if let Some(u) = n.as_u64() {
                CborValue::Integer(Integer::from(u))
            } else {
                CborValue::Float(n.as_f64().expect("finite f64") as f64)
            }
        }
        Value::String(s) => CborValue::Text(s.clone()),
        Value::Array(items) => CborValue::Array(items.iter().map(json_to_cbor).collect()),
        Value::Object(map) => {
            let mut out = Vec::with_capacity(map.len());
            for (k, v) in map {
                out.push((CborValue::Text(k.clone()), json_to_cbor(v)));
            }
            CborValue::Map(out)
        }
    }
}

#[derive(Default)]
struct PatchWriter {
    bytes: Vec<u8>,
    patch_sid: u64,
}

impl PatchWriter {
    fn new(patch_sid: u64) -> Self {
        Self {
            bytes: Vec::new(),
            patch_sid,
        }
    }

    fn vu57(&mut self, mut value: u64) {
        for _ in 0..7 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(b);
                return;
            }
            b |= 0x80;
            self.bytes.push(b);
        }
        self.bytes.push((value & 0xff) as u8);
    }

    fn b1vu56(&mut self, flag: u8, mut value: u64) {
        let low6 = (value & 0x3f) as u8;
        value >>= 6;
        let mut first = (flag << 7) | low6;
        if value == 0 {
            self.bytes.push(first);
            return;
        }
        first |= 0x40;
        self.bytes.push(first);

        for _ in 0..6 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(b);
                return;
            }
            b |= 0x80;
            self.bytes.push(b);
        }

        self.bytes.push((value & 0xff) as u8);
    }

    fn encode_id(&mut self, sid: u64, time: u64) {
        if sid == self.patch_sid {
            self.b1vu56(0, time);
        } else {
            self.b1vu56(1, time);
            self.vu57(sid);
        }
    }

    fn push_cbor(&mut self, value: &CborValue) {
        ciborium::ser::into_writer(value, &mut self.bytes).expect("CBOR encode must succeed");
    }

    fn write_op_len(&mut self, opcode: u8, len: u64) {
        if len <= 0b111 {
            self.bytes.push((opcode << 3) | (len as u8));
        } else {
            self.bytes.push(opcode << 3);
            self.vu57(len);
        }
    }
}

fn encode_canonical_patch(input: &Value) -> Vec<u8> {
    let sid = input["sid"].as_u64().expect("input.sid must be u64");
    let time = input["time"].as_u64().expect("input.time must be u64");
    let meta_kind = input["meta_kind"]
        .as_str()
        .unwrap_or("undefined");
    let ops = input["ops"].as_array().expect("input.ops must be array");

    let mut w = PatchWriter::new(sid);
    w.vu57(sid);
    w.vu57(time);

    if meta_kind == "undefined" {
        // CBOR undefined
        w.bytes.push(0xf7);
    } else {
        panic!("unsupported meta_kind: {meta_kind}");
    }

    w.vu57(ops.len() as u64);

    for op in ops {
        let op_name = as_str(&op["op"], "op.op");
        match op_name {
            "new_con" => {
                w.bytes.push(0 << 3);
                let cbor = json_to_cbor(&op["value"]);
                w.push_cbor(&cbor);
            }
            "new_val" => {
                w.bytes.push(1 << 3);
            }
            "new_obj" => {
                w.bytes.push(2 << 3);
            }
            "new_str" => {
                w.bytes.push(4 << 3);
            }
            "new_arr" => {
                w.bytes.push(6 << 3);
            }
            "ins_val" => {
                w.bytes.push(9 << 3);
                let (obj_sid, obj_time) = as_ts(&op["obj"], "op.obj");
                let (val_sid, val_time) = as_ts(&op["val"], "op.val");
                w.encode_id(obj_sid, obj_time);
                w.encode_id(val_sid, val_time);
            }
            "ins_obj" => {
                let tuples = op["data"].as_array().expect("op.data must be array");
                w.write_op_len(10, tuples.len() as u64);
                let (obj_sid, obj_time) = as_ts(&op["obj"], "op.obj");
                w.encode_id(obj_sid, obj_time);
                for tup in tuples {
                    let t = tup.as_array().expect("tuple must be [key,id]");
                    assert_eq!(t.len(), 2, "tuple must have 2 elements");
                    let key = as_str(&t[0], "tuple key");
                    w.push_cbor(&CborValue::Text(key.to_string()));
                    let (sid, time) = as_ts(&t[1], "tuple id");
                    w.encode_id(sid, time);
                }
            }
            "ins_str" => {
                let (obj_sid, obj_time) = as_ts(&op["obj"], "op.obj");
                let (ref_sid, ref_time) = as_ts(&op["ref"], "op.ref");
                let data = as_str(&op["data"], "op.data");
                let data_bytes = data.as_bytes();
                w.write_op_len(12, data_bytes.len() as u64);
                w.encode_id(obj_sid, obj_time);
                w.encode_id(ref_sid, ref_time);
                w.bytes.extend_from_slice(data_bytes);
            }
            "ins_arr" => {
                let vals = op["data"].as_array().expect("op.data must be array");
                w.write_op_len(14, vals.len() as u64);
                let (obj_sid, obj_time) = as_ts(&op["obj"], "op.obj");
                let (ref_sid, ref_time) = as_ts(&op["ref"], "op.ref");
                w.encode_id(obj_sid, obj_time);
                w.encode_id(ref_sid, ref_time);
                for id in vals {
                    let (sid, time) = as_ts(id, "ins_arr id");
                    w.encode_id(sid, time);
                }
            }
            "nop" => {
                let len = as_u64(&op["len"], "op.len");
                w.write_op_len(17, len);
            }
            other => panic!("unsupported canonical op: {other}"),
        }
    }

    w.bytes
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

        let encoded = encode_canonical_patch(&fixture["input"]);
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
            .map(|v| u8::try_from(v.as_u64().expect("opcode must be u64")).expect("opcode must fit u8"))
            .collect();

        assert_eq!(patch.op_count(), expected_op_count);
        assert_eq!(patch.span(), expected_span);
        assert_eq!(patch.opcodes(), expected_opcodes.as_slice());
    }

    assert!(seen >= 4, "expected at least 4 patch_canonical_encode fixtures");
}
