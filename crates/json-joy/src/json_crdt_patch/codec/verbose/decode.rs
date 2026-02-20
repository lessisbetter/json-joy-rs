//! Verbose JSON codec decoder.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/verbose/decode.ts`.

use crate::json_crdt_patch::clock::{ts, tss, ClockVector, ServerClockVector, Ts};
use crate::json_crdt_patch::enums::SESSION;
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use serde_json::Value;

fn decode_id(v: &Value) -> Ts {
    match v {
        Value::Number(n) => ts(SESSION::SERVER, n.as_u64().unwrap_or(0)),
        Value::Array(arr) if arr.len() >= 2 => {
            let sid = arr[0].as_u64().unwrap_or(0);
            let time = arr[1].as_u64().unwrap_or(0);
            ts(sid, time)
        }
        _ => ts(SESSION::SERVER, 0),
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

/// Decodes a verbose-format JSON value into a [`Patch`].
pub fn decode(data: &Value) -> Patch {
    let obj = match data.as_object() {
        Some(o) => o,
        None => panic!("INVALID_PATCH"),
    };

    let id_val = obj.get("id").expect("missing id");
    let clock = match id_val {
        Value::Number(n) => {
            let time = n.as_u64().unwrap_or(0);
            let cv = ServerClockVector::new(time);
            PatchBuilder::from_server_clock(cv)
        }
        Value::Array(arr) if arr.len() >= 2 => {
            let sid = arr[0].as_u64().unwrap_or(0);
            let time = arr[1].as_u64().unwrap_or(0);
            let cv = ClockVector::new(sid, time);
            PatchBuilder::from_clock_vector(cv)
        }
        _ => panic!("INVALID_ID"),
    };
    let mut builder = clock;

    let ops = obj
        .get("ops")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    for op_val in ops {
        let op_obj = match op_val.as_object() {
            Some(o) => o,
            None => continue,
        };
        let op_name = match op_obj.get("op").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        match op_name {
            "new_con" => {
                if op_obj
                    .get("timestamp")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let ref_id = decode_id(op_obj.get("value").unwrap_or(&Value::Null));
                    builder.con_ref(ref_id);
                } else {
                    let val = json_to_pack(op_obj.get("value").unwrap_or(&Value::Null));
                    builder.con_val(val);
                }
            }
            "new_val" => {
                builder.val();
            }
            "new_obj" => {
                builder.obj();
            }
            "new_vec" => {
                builder.vec();
            }
            "new_str" => {
                builder.str_node();
            }
            "new_bin" => {
                builder.bin();
            }
            "new_arr" => {
                builder.arr();
            }
            "ins_val" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let val = decode_id(op_obj.get("value").unwrap_or(&Value::Null));
                builder.set_val(obj, val);
            }
            "ins_obj" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let value = op_obj
                    .get("value")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);
                let tuples: Vec<(String, Ts)> = value
                    .iter()
                    .filter_map(|pair| {
                        let arr = pair.as_array()?;
                        let key = arr.first()?.as_str()?.to_owned();
                        let id = decode_id(arr.get(1)?);
                        Some((key, id))
                    })
                    .collect();
                if !tuples.is_empty() {
                    builder.ins_obj(obj, tuples);
                }
            }
            "ins_vec" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let value = op_obj
                    .get("value")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);
                let tuples: Vec<(u8, Ts)> = value
                    .iter()
                    .filter_map(|pair| {
                        let arr = pair.as_array()?;
                        let idx = arr.first()?.as_u64()? as u8;
                        let id = decode_id(arr.get(1)?);
                        Some((idx, id))
                    })
                    .collect();
                if !tuples.is_empty() {
                    builder.ins_vec(obj, tuples);
                }
            }
            "ins_str" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let after = op_obj.get("after").map(decode_id).unwrap_or(obj);
                let data = op_obj
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                if !data.is_empty() {
                    builder.ins_str(obj, after, data);
                }
            }
            "ins_bin" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let after = op_obj.get("after").map(decode_id).unwrap_or(obj);
                let b64 = op_obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap_or_default();
                if !data.is_empty() {
                    builder.ins_bin(obj, after, data);
                }
            }
            "ins_arr" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let after = op_obj.get("after").map(decode_id).unwrap_or(obj);
                let values = op_obj
                    .get("values")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);
                let elems: Vec<Ts> = values.iter().map(decode_id).collect();
                builder.ins_arr(obj, after, elems);
            }
            "upd_arr" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let after = decode_id(op_obj.get("ref").unwrap_or(&Value::Null));
                let val = decode_id(op_obj.get("value").unwrap_or(&Value::Null));
                builder.upd_arr(obj, after, val);
            }
            "del" => {
                let obj = decode_id(op_obj.get("obj").unwrap_or(&Value::Null));
                let what_arr = op_obj
                    .get("what")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);
                let what: Vec<crate::json_crdt_patch::clock::Tss> = what_arr
                    .iter()
                    .filter_map(|s| {
                        let arr = s.as_array()?;
                        let sid = arr.first()?.as_u64()?;
                        let time = arr.get(1)?.as_u64()?;
                        let span = arr.get(2)?.as_u64()?;
                        Some(tss(sid, time, span))
                    })
                    .collect();
                builder.del(obj, what);
            }
            "nop" => {
                let len = op_obj.get("len").and_then(|v| v.as_u64()).unwrap_or(1);
                builder.nop(len);
            }
            _ => {}
        }
    }

    let mut patch = builder.flush();
    if let Some(meta_val) = obj.get("meta") {
        patch.meta = Some(json_to_pack(meta_val));
    }
    patch
}
