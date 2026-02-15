use std::fs;
use std::path::{Path, PathBuf};

use ciborium::value::{Integer, Value as CborValue};
use json_joy_core::model::Model;
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

fn json_scalar_to_cbor(v: &Value) -> CborValue {
    match v {
        Value::Null => CborValue::Null,
        Value::Bool(b) => CborValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CborValue::Integer(Integer::from(i))
            } else if let Some(u) = n.as_u64() {
                CborValue::Integer(Integer::from(u))
            } else {
                panic!("canonical model encoder supports integer numbers only");
            }
        }
        Value::String(s) => CborValue::Text(s.clone()),
        _ => panic!("canonical model encoder supports scalar CBOR values only"),
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, b: u8) {
        self.bytes.push(b);
    }

    fn buf(&mut self, data: &[u8]) {
        self.bytes.extend_from_slice(data);
    }

    fn vu57(&mut self, mut value: u64) {
        for _ in 0..7 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.u8(b);
                return;
            }
            b |= 0x80;
            self.u8(b);
        }
        self.u8((value & 0xff) as u8);
    }

    fn b1vu56(&mut self, flag: u8, mut value: u64) {
        let low6 = (value & 0x3f) as u8;
        value >>= 6;
        let mut first = ((flag & 1) << 7) | low6;
        if value == 0 {
            self.u8(first);
            return;
        }

        first |= 0x40;
        self.u8(first);
        for _ in 0..6 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.u8(b);
                return;
            }
            b |= 0x80;
            self.u8(b);
        }
        self.u8((value & 0xff) as u8);
    }
}

fn write_type_len(w: &mut Writer, major: u8, len: u64) {
    if len < 31 {
        w.u8((major << 5) | (len as u8));
    } else {
        w.u8((major << 5) | 31);
        w.vu57(len);
    }
}

fn encode_model_canonical(input: &Value) -> Vec<u8> {
    let mode = as_str(&input["mode"], "input.mode");
    let mut root_writer = Writer::default();

    let mut clock_table: Vec<(u64, u64)> = Vec::new();
    if mode == "logical" {
        let entries = input["clock_table"]
            .as_array()
            .expect("input.clock_table must be array for logical mode");
        for e in entries {
            clock_table.push(as_ts(e, "clock_table entry"));
        }
        assert!(!clock_table.is_empty(), "clock_table must not be empty");
    }

    let encode_id = |w: &mut Writer, id: (u64, u64)| {
        if mode == "server" {
            w.vu57(id.1);
            return;
        }

        let (sid, time) = id;
        let (idx, base) = clock_table
            .iter()
            .enumerate()
            .find(|(_, t)| t.0 == sid)
            .map(|(i, t)| (i as u64, t.1))
            .unwrap_or_else(|| panic!("sid {} missing from clock_table", sid));

        let diff = time
            .checked_sub(base)
            .unwrap_or_else(|| panic!("time {} < base {} for sid {}", time, base, sid));
        if idx <= 7 && diff <= 15 {
            w.u8(((idx as u8) << 4) | (diff as u8));
        } else {
            w.b1vu56(0, idx);
            w.vu57(diff);
        }
    };

    fn encode_node(
        w: &mut Writer,
        node: &Value,
        encode_id: &dyn Fn(&mut Writer, (u64, u64)),
    ) {
        let id = as_ts(&node["id"], "node.id");
        encode_id(w, id);

        let kind = as_str(&node["kind"], "node.kind");
        match kind {
            "con" => {
                w.u8(0b0000_0000);
                let cbor = json_scalar_to_cbor(&node["value"]);
                ciborium::ser::into_writer(&cbor, &mut w.bytes).expect("CBOR encode must succeed");
            }
            "val" => {
                w.u8(0b0010_0000);
                encode_node(w, &node["child"], encode_id);
            }
            "obj" => {
                let entries = node["entries"]
                    .as_array()
                    .expect("obj.entries must be array");
                write_type_len(w, 2, entries.len() as u64);
                for e in entries {
                    let key = as_str(&e["key"], "obj entry key");
                    let key_cbor = CborValue::Text(key.to_string());
                    ciborium::ser::into_writer(&key_cbor, &mut w.bytes).expect("CBOR key encode must succeed");
                    encode_node(w, &e["value"], encode_id);
                }
            }
            "vec" => {
                let vals = node["values"].as_array().expect("vec.values must be array");
                write_type_len(w, 3, vals.len() as u64);
                for v in vals {
                    if v.is_null() {
                        w.u8(0);
                    } else {
                        encode_node(w, v, encode_id);
                    }
                }
            }
            "str" => {
                let chunks = node["chunks"].as_array().expect("str.chunks must be array");
                write_type_len(w, 4, chunks.len() as u64);
                for ch in chunks {
                    let cid = as_ts(&ch["id"], "str chunk id");
                    encode_id(w, cid);
                    if ch.get("text").is_some() {
                        let cbor = CborValue::Text(as_str(&ch["text"], "str chunk text").to_string());
                        ciborium::ser::into_writer(&cbor, &mut w.bytes).expect("CBOR str encode must succeed");
                    } else {
                        let del = as_u64(&ch["deleted"], "str chunk deleted");
                        let cbor = CborValue::Integer(Integer::from(del));
                        ciborium::ser::into_writer(&cbor, &mut w.bytes).expect("CBOR del encode must succeed");
                    }
                }
            }
            "bin" => {
                let chunks = node["chunks"].as_array().expect("bin.chunks must be array");
                write_type_len(w, 5, chunks.len() as u64);
                for ch in chunks {
                    let cid = as_ts(&ch["id"], "bin chunk id");
                    encode_id(w, cid);
                    if ch.get("deleted").is_some() {
                        w.b1vu56(1, as_u64(&ch["deleted"], "bin chunk deleted"));
                    } else {
                        let b = decode_hex(as_str(&ch["bytes_hex"], "bin chunk bytes_hex"));
                        w.b1vu56(0, b.len() as u64);
                        w.buf(&b);
                    }
                }
            }
            "arr" => {
                let chunks = node["chunks"].as_array().expect("arr.chunks must be array");
                write_type_len(w, 6, chunks.len() as u64);
                for ch in chunks {
                    let cid = as_ts(&ch["id"], "arr chunk id");
                    encode_id(w, cid);
                    if ch.get("deleted").is_some() {
                        w.b1vu56(1, as_u64(&ch["deleted"], "arr chunk deleted"));
                    } else {
                        let vals = ch["values"].as_array().expect("arr chunk values must be array");
                        w.b1vu56(0, vals.len() as u64);
                        for v in vals {
                            encode_node(w, v, encode_id);
                        }
                    }
                }
            }
            other => panic!("unsupported canonical node kind: {other}"),
        }
    }

    encode_node(&mut root_writer, &input["root"], &encode_id);

    if mode == "server" {
        let mut out = Writer::default();
        out.u8(0x80);
        out.vu57(as_u64(&input["server_time"], "input.server_time"));
        out.buf(&root_writer.bytes);
        return out.bytes;
    }

    let mut out = Writer::default();
    let root_len = root_writer.bytes.len() as u32;
    out.u8((root_len >> 24) as u8);
    out.u8((root_len >> 16) as u8);
    out.u8((root_len >> 8) as u8);
    out.u8(root_len as u8);
    out.buf(&root_writer.bytes);
    out.vu57(clock_table.len() as u64);
    for (sid, time) in clock_table {
        out.vu57(sid);
        out.vu57(time);
    }
    out.bytes
}

#[test]
fn model_canonical_encode_fixtures_match_oracle_binary() {
    let dir = fixtures_dir();
    let manifest = read_json(&dir.join("manifest.json"));
    let fixtures = manifest["fixtures"].as_array().expect("manifest.fixtures must be array");

    let mut seen = 0u32;
    let mut seen_server = 0u32;
    let mut seen_logical = 0u32;
    for entry in fixtures {
        if entry["scenario"].as_str() != Some("model_canonical_encode") {
            continue;
        }
        seen += 1;

        let file = entry["file"].as_str().expect("fixture entry file must be string");
        let fixture = read_json(&dir.join(file));

        let mode = as_str(&fixture["input"]["mode"], "input.mode");
        if mode == "server" {
            seen_server += 1;
        } else if mode == "logical" {
            seen_logical += 1;
        }

        let encoded = encode_model_canonical(&fixture["input"]);
        let expected = decode_hex(
            fixture["expected"]["model_binary_hex"]
                .as_str()
                .expect("expected.model_binary_hex must be string"),
        );

        assert_eq!(
            encoded, expected,
            "canonical model encode byte mismatch for fixture {}",
            entry["name"]
        );

        let model = Model::from_binary(&encoded)
            .unwrap_or_else(|e| panic!("encoded canonical model failed to decode for {}: {e}", entry["name"]));
        assert_eq!(
            model.view(),
            &fixture["expected"]["view_json"],
            "canonical model view mismatch for fixture {}",
            entry["name"]
        );
    }

    assert!(seen >= 6, "expected at least 6 model_canonical_encode fixtures");
    assert!(seen_logical >= 3, "expected at least 3 logical canonical encode fixtures");
    assert!(seen_server >= 2, "expected at least 2 server canonical encode fixtures");
}
