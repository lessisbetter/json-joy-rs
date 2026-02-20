//! Compact JSON codec decoder.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/compact/decode.ts`.

use crate::json_crdt_patch::clock::{ts, tss, ClockVector, ServerClockVector, Ts};
use crate::json_crdt_patch::enums::{JsonCrdtPatchOpcode, SESSION};
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use serde_json::Value;

fn decode_id(v: &Value, patch_sid: u64) -> Ts {
    match v {
        Value::Number(n) => ts(patch_sid, n.as_u64().unwrap_or(0)),
        Value::Array(arr) if arr.len() >= 2 => {
            let sid = arr[0].as_u64().unwrap_or(0);
            let time = arr[1].as_u64().unwrap_or(0);
            ts(sid, time)
        }
        _ => ts(patch_sid, 0),
    }
}

fn decode_tss(v: &Value, patch_sid: u64) -> Option<crate::json_crdt_patch::clock::Tss> {
    let a = v.as_array()?;
    match a.len() {
        3 => {
            let sid = a.first()?.as_u64()?;
            let time = a.get(1)?.as_u64()?;
            let span = a.get(2)?.as_u64()?;
            Some(tss(sid, time, span))
        }
        2 => {
            let time = a.first()?.as_u64()?;
            let span = a.get(1)?.as_u64()?;
            Some(tss(patch_sid, time, span))
        }
        _ => None,
    }
}

fn json_to_pack(v: &Value) -> json_joy_json_pack::PackValue {
    use json_joy_json_pack::PackValue;
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                PackValue::UInteger(u)
            } else {
                PackValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => PackValue::Str(s.clone()),
        Value::Array(arr) => PackValue::Array(arr.iter().map(json_to_pack).collect()),
        Value::Object(obj) => PackValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_pack(v)))
                .collect(),
        ),
    }
}

/// Decodes a compact-format array into a [`Patch`].
pub fn decode(data: &[Value]) -> Patch {
    if data.is_empty() {
        panic!("INVALID_PATCH");
    }

    // First element is the header: [id, meta?]
    let header = data[0].as_array().expect("INVALID_HEADER");
    let id_val = header.first().expect("MISSING_ID");

    let (patch_sid, patch_time) = match id_val {
        Value::Number(n) => (SESSION::SERVER, n.as_u64().unwrap_or(0)),
        Value::Array(arr) if arr.len() >= 2 => {
            (arr[0].as_u64().unwrap_or(0), arr[1].as_u64().unwrap_or(0))
        }
        _ => panic!("INVALID_ID"),
    };

    let mut builder = if patch_sid == SESSION::SERVER {
        PatchBuilder::from_server_clock(ServerClockVector::new(patch_time))
    } else {
        PatchBuilder::from_clock_vector(ClockVector::new(patch_sid, patch_time))
    };

    if let Some(meta_val) = header.get(1) {
        builder.patch.meta = Some(json_to_pack(meta_val));
    }

    // Remaining elements are operations
    for op_val in &data[1..] {
        let arr = match op_val.as_array() {
            Some(a) => a,
            None => continue,
        };
        let opcode_num = match arr.first().and_then(|v| v.as_u64()) {
            Some(n) => n as u8,
            None => continue,
        };

        match JsonCrdtPatchOpcode::from_u8(opcode_num) {
            Some(JsonCrdtPatchOpcode::NewCon) => {
                // [0] or [0, value] or [0, ts_ref, true]
                let is_ts_ref = arr.get(2).and_then(Value::as_bool).unwrap_or(false);
                if is_ts_ref {
                    let ref_id = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                    builder.con_ref(ref_id);
                } else {
                    let val = arr
                        .get(1)
                        .map(json_to_pack)
                        .unwrap_or(json_joy_json_pack::PackValue::Undefined);
                    builder.con_val(val);
                }
            }
            Some(JsonCrdtPatchOpcode::NewVal) => {
                builder.val();
            }
            Some(JsonCrdtPatchOpcode::NewObj) => {
                builder.obj();
            }
            Some(JsonCrdtPatchOpcode::NewVec) => {
                builder.vec();
            }
            Some(JsonCrdtPatchOpcode::NewStr) => {
                builder.str_node();
            }
            Some(JsonCrdtPatchOpcode::NewBin) => {
                builder.bin();
            }
            Some(JsonCrdtPatchOpcode::NewArr) => {
                builder.arr();
            }
            Some(JsonCrdtPatchOpcode::InsVal) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let val = decode_id(arr.get(2).unwrap_or(&Value::Null), patch_sid);
                builder.set_val(obj, val);
            }
            Some(JsonCrdtPatchOpcode::InsObj) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let mut tuples = Vec::new();
                if let Some(items) = arr.get(2).and_then(Value::as_array) {
                    for item in items {
                        if let Some(pair) = item.as_array() {
                            if pair.len() >= 2 {
                                if let Some(key) = pair[0].as_str() {
                                    let val_id = decode_id(&pair[1], patch_sid);
                                    tuples.push((key.to_owned(), val_id));
                                }
                            }
                        }
                    }
                }
                if !tuples.is_empty() {
                    builder.ins_obj(obj, tuples);
                }
            }
            Some(JsonCrdtPatchOpcode::InsVec) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let mut tuples = Vec::new();
                if let Some(items) = arr.get(2).and_then(Value::as_array) {
                    for item in items {
                        if let Some(pair) = item.as_array() {
                            if pair.len() >= 2 {
                                if let Some(idx) = pair[0].as_u64() {
                                    let val_id = decode_id(&pair[1], patch_sid);
                                    tuples.push((idx as u8, val_id));
                                }
                            }
                        }
                    }
                }
                if !tuples.is_empty() {
                    builder.ins_vec(obj, tuples);
                }
            }
            Some(JsonCrdtPatchOpcode::InsStr) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let after = decode_id(arr.get(2).unwrap_or(&Value::Null), patch_sid);
                let data = arr.get(3).and_then(|v| v.as_str()).unwrap_or("").to_owned();
                if !data.is_empty() {
                    builder.ins_str(obj, after, data);
                }
            }
            Some(JsonCrdtPatchOpcode::InsBin) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let after = decode_id(arr.get(2).unwrap_or(&Value::Null), patch_sid);
                let b64 = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap_or_default();
                if !data.is_empty() {
                    builder.ins_bin(obj, after, data);
                }
            }
            Some(JsonCrdtPatchOpcode::InsArr) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let after = decode_id(arr.get(2).unwrap_or(&Value::Null), patch_sid);
                let elems: Vec<Ts> = arr
                    .get(3)
                    .and_then(Value::as_array)
                    .map(|items| items.iter().map(|e| decode_id(e, patch_sid)).collect())
                    .unwrap_or_default();
                builder.ins_arr(obj, after, elems);
            }
            Some(JsonCrdtPatchOpcode::UpdArr) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let after = decode_id(arr.get(2).unwrap_or(&Value::Null), patch_sid);
                let val = decode_id(arr.get(3).unwrap_or(&Value::Null), patch_sid);
                builder.upd_arr(obj, after, val);
            }
            Some(JsonCrdtPatchOpcode::Del) => {
                let obj = decode_id(arr.get(1).unwrap_or(&Value::Null), patch_sid);
                let what: Vec<crate::json_crdt_patch::clock::Tss> = arr
                    .get(2)
                    .and_then(Value::as_array)
                    .map(|spans| {
                        spans
                            .iter()
                            .filter_map(|span| decode_tss(span, patch_sid))
                            .collect()
                    })
                    .unwrap_or_default();
                builder.del(obj, what);
            }
            Some(JsonCrdtPatchOpcode::Nop) => {
                let len = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(1);
                builder.nop(len);
            }
            _ => {}
        }
    }

    builder.flush()
}
