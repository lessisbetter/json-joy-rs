//! Compact JSON codec encoder.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/compact/encode.ts`.

use crate::json_crdt_patch::clock::{Ts, Tss};
use crate::json_crdt_patch::enums::{JsonCrdtPatchOpcode, SESSION};
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;
use serde_json::{json, Value};

fn encode_ts(id: Ts, patch_sid: u64) -> Value {
    if id.sid == patch_sid {
        json!(id.time)
    } else if id.sid == SESSION::SERVER {
        json!(id.time)
    } else {
        json!([id.sid, id.time])
    }
}

fn encode_tss(tss: &Tss) -> Value {
    json!([tss.sid, tss.time, tss.span])
}

fn pack_to_json(v: &json_joy_json_pack::PackValue) -> Value {
    use json_joy_json_pack::PackValue;
    match v {
        PackValue::Null | PackValue::Undefined | PackValue::Blob(_) => Value::Null,
        PackValue::Bool(b) => json!(b),
        PackValue::Integer(i) => json!(i),
        PackValue::UInteger(u) => json!(u),
        PackValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        PackValue::BigInt(i) => json!(i),
        PackValue::Str(s) => json!(s),
        PackValue::Bytes(b) => {
            use base64::Engine;
            Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        PackValue::Array(arr) => Value::Array(arr.iter().map(pack_to_json).collect()),
        PackValue::Object(obj) => {
            let map: serde_json::Map<_, _> = obj
                .iter()
                .map(|(k, v)| (k.clone(), pack_to_json(v)))
                .collect();
            Value::Object(map)
        }
        PackValue::Extension(_) => Value::Null,
    }
}

/// Encodes a [`Patch`] into the compact format (a `Vec<serde_json::Value>`).
pub fn encode(patch: &Patch) -> Vec<Value> {
    let id = patch.get_id().expect("PATCH_EMPTY");
    let patch_sid = id.sid;

    // Header: [id, meta?]
    let id_val = if id.sid == SESSION::SERVER {
        json!(id.time)
    } else {
        json!([id.sid, id.time])
    };
    let header = match &patch.meta {
        None => json!([id_val]),
        Some(meta) => json!([id_val, pack_to_json(meta)]),
    };

    let mut out = vec![header];

    for op in &patch.ops {
        let op_val = match op {
            Op::NewCon { val, .. } => match val {
                ConValue::Ref(ts_ref) => json!([
                    JsonCrdtPatchOpcode::NewCon as u8,
                    1,
                    encode_ts(*ts_ref, patch_sid)
                ]),
                ConValue::Val(v) => {
                    let encoded = pack_to_json(v);
                    if encoded == Value::Null {
                        // Don't include undefined/null — omit value field
                        json!([JsonCrdtPatchOpcode::NewCon as u8])
                    } else {
                        json!([JsonCrdtPatchOpcode::NewCon as u8, encoded])
                    }
                }
            },
            Op::NewVal { .. } => json!([JsonCrdtPatchOpcode::NewVal as u8]),
            Op::NewObj { .. } => json!([JsonCrdtPatchOpcode::NewObj as u8]),
            Op::NewVec { .. } => json!([JsonCrdtPatchOpcode::NewVec as u8]),
            Op::NewStr { .. } => json!([JsonCrdtPatchOpcode::NewStr as u8]),
            Op::NewBin { .. } => json!([JsonCrdtPatchOpcode::NewBin as u8]),
            Op::NewArr { .. } => json!([JsonCrdtPatchOpcode::NewArr as u8]),
            Op::InsVal { obj, val, .. } => json!([
                JsonCrdtPatchOpcode::InsVal as u8,
                encode_ts(*obj, patch_sid),
                encode_ts(*val, patch_sid),
            ]),
            Op::InsObj { obj, data, .. } => {
                let pairs: Vec<Value> = data
                    .iter()
                    .flat_map(|(k, v)| vec![json!(k), encode_ts(*v, patch_sid)])
                    .collect();
                let mut v = vec![
                    json!(JsonCrdtPatchOpcode::InsObj as u8),
                    encode_ts(*obj, patch_sid),
                ];
                v.extend(pairs);
                Value::Array(v)
            }
            Op::InsVec { obj, data, .. } => {
                let pairs: Vec<Value> = data
                    .iter()
                    .flat_map(|(k, v)| vec![json!(k), encode_ts(*v, patch_sid)])
                    .collect();
                let mut v = vec![
                    json!(JsonCrdtPatchOpcode::InsVec as u8),
                    encode_ts(*obj, patch_sid),
                ];
                v.extend(pairs);
                Value::Array(v)
            }
            Op::InsStr {
                obj, after, data, ..
            } => json!([
                JsonCrdtPatchOpcode::InsStr as u8,
                encode_ts(*obj, patch_sid),
                encode_ts(*after, patch_sid),
                data,
            ]),
            Op::InsBin {
                obj, after, data, ..
            } => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                json!([
                    JsonCrdtPatchOpcode::InsBin as u8,
                    encode_ts(*obj, patch_sid),
                    encode_ts(*after, patch_sid),
                    b64,
                ])
            }
            Op::InsArr {
                obj, after, data, ..
            } => {
                let elems: Vec<Value> = data.iter().map(|e| encode_ts(*e, patch_sid)).collect();
                let mut v = vec![
                    json!(JsonCrdtPatchOpcode::InsArr as u8),
                    encode_ts(*obj, patch_sid),
                    encode_ts(*after, patch_sid),
                ];
                v.extend(elems);
                Value::Array(v)
            }
            Op::UpdArr {
                obj, after, val, ..
            } => json!([
                JsonCrdtPatchOpcode::UpdArr as u8,
                encode_ts(*obj, patch_sid),
                encode_ts(*after, patch_sid),
                encode_ts(*val, patch_sid),
            ]),
            Op::Del { obj, what, .. } => {
                let spans: Vec<Value> = what.iter().map(encode_tss).collect();
                let mut v = vec![
                    json!(JsonCrdtPatchOpcode::Del as u8),
                    encode_ts(*obj, patch_sid),
                ];
                v.extend(spans);
                Value::Array(v)
            }
            Op::Nop { len, .. } => {
                if *len > 1 {
                    json!([JsonCrdtPatchOpcode::Nop as u8, len])
                } else {
                    json!([JsonCrdtPatchOpcode::Nop as u8])
                }
            }
        };
        out.push(op_val);
    }

    out
}
