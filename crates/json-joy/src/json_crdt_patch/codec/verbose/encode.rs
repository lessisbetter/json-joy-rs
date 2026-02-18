//! Verbose JSON codec encoder.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/verbose/encode.ts`.

use serde_json::{json, Value};
use crate::json_crdt_patch::clock::{Ts};
use crate::json_crdt_patch::enums::SESSION;
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;

fn encode_ts(id: Ts) -> Value {
    if id.sid == SESSION::SERVER {
        json!(id.time)
    } else {
        json!([id.sid, id.time])
    }
}

fn encode_tss(tss: &crate::json_crdt_patch::clock::Tss) -> Value {
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
            // Encode bytes as base64
            use base64::Engine;
            Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        PackValue::Array(arr) => Value::Array(arr.iter().map(pack_to_json).collect()),
        PackValue::Object(obj) => {
            let map: serde_json::Map<_, _> = obj.iter()
                .map(|(k, v)| (k.clone(), pack_to_json(v)))
                .collect();
            Value::Object(map)
        }
        PackValue::Extension(ext) => json!(null), // not representable in plain JSON
    }
}

/// Encodes a [`Patch`] into the verbose JSON format (a `serde_json::Value`).
pub fn encode(patch: &Patch) -> Value {
    let id = patch.get_id().expect("PATCH_EMPTY");
    let mut ops_arr = Vec::new();

    for op in &patch.ops {
        let op_val = match op {
            Op::NewCon { val, .. } => match val {
                ConValue::Ref(ts_ref) => json!({
                    "op": "new_con",
                    "timestamp": true,
                    "value": encode_ts(*ts_ref),
                }),
                ConValue::Val(v) => json!({
                    "op": "new_con",
                    "value": pack_to_json(v),
                }),
            },
            Op::NewVal { .. } => json!({"op": "new_val"}),
            Op::NewObj { .. } => json!({"op": "new_obj"}),
            Op::NewVec { .. } => json!({"op": "new_vec"}),
            Op::NewStr { .. } => json!({"op": "new_str"}),
            Op::NewBin { .. } => json!({"op": "new_bin"}),
            Op::NewArr { .. } => json!({"op": "new_arr"}),
            Op::InsVal { obj, val, .. } => json!({
                "op": "ins_val",
                "obj": encode_ts(*obj),
                "value": encode_ts(*val),
            }),
            Op::InsObj { obj, data, .. } => {
                let vals: Vec<Value> = data.iter()
                    .map(|(k, v)| json!([k, encode_ts(*v)]))
                    .collect();
                json!({"op": "ins_obj", "obj": encode_ts(*obj), "value": vals})
            }
            Op::InsVec { obj, data, .. } => {
                let vals: Vec<Value> = data.iter()
                    .map(|(k, v)| json!([k, encode_ts(*v)]))
                    .collect();
                json!({"op": "ins_vec", "obj": encode_ts(*obj), "value": vals})
            }
            Op::InsStr { obj, after, data, .. } => json!({
                "op": "ins_str",
                "obj": encode_ts(*obj),
                "after": encode_ts(*after),
                "value": data,
            }),
            Op::InsBin { obj, after, data, .. } => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                json!({"op": "ins_bin", "obj": encode_ts(*obj), "after": encode_ts(*after), "value": b64})
            }
            Op::InsArr { obj, after, data, .. } => {
                let vals: Vec<Value> = data.iter().map(|v| encode_ts(*v)).collect();
                json!({"op": "ins_arr", "obj": encode_ts(*obj), "after": encode_ts(*after), "values": vals})
            }
            Op::UpdArr { obj, after, val, .. } => json!({
                "op": "upd_arr",
                "obj": encode_ts(*obj),
                "ref": encode_ts(*after),
                "value": encode_ts(*val),
            }),
            Op::Del { obj, what, .. } => {
                let spans: Vec<Value> = what.iter().map(encode_tss).collect();
                json!({"op": "del", "obj": encode_ts(*obj), "what": spans})
            }
            Op::Nop { len, .. } => {
                if *len > 1 {
                    json!({"op": "nop", "len": len})
                } else {
                    json!({"op": "nop"})
                }
            }
        };
        ops_arr.push(op_val);
    }

    let mut res = serde_json::Map::new();
    res.insert("id".into(), json!([id.sid, id.time]));
    res.insert("ops".into(), Value::Array(ops_arr));
    if let Some(meta) = &patch.meta {
        res.insert("meta".into(), pack_to_json(meta));
    }
    Value::Object(res)
}
