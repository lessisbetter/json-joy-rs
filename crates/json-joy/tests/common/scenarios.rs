#![allow(dead_code)]

use json_joy::json_crdt::codec::indexed::binary as indexed_binary;
use json_joy::json_crdt::codec::sidecar::binary as sidecar_binary;
use json_joy::json_crdt::codec::structural::binary as structural_binary;
use json_joy::json_crdt::constants::ORIGIN;
use json_joy::json_crdt::model::Model;
use json_joy::json_crdt::nodes::{CrdtNode, TsKey, ValNode};
use json_joy::json_crdt_diff::diff_node;
use json_joy::json_crdt_patch::clock::{ts, tss, Ts};
use json_joy::json_crdt_patch::codec::{compact, compact_binary, verbose};
use json_joy::json_crdt_patch::compaction;
use json_joy::json_crdt_patch::operations::{ConValue, Op};
use json_joy::json_crdt_patch::patch::Patch;
use json_joy::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy::util_inner::diff::{bin as bin_diff, line as line_diff, str as str_diff};
use json_joy_json_pack::PackValue;
use serde_json::{json, Map, Value};

use crate::common::assertions::{decode_hex, encode_hex, op_to_opcode};

fn json_to_pack(v: &Value) -> PackValue {
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                PackValue::UInteger(u)
            } else if let Some(f) = n.as_f64() {
                PackValue::Float(f)
            } else {
                PackValue::Null
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

fn build_json_val(builder: &mut PatchBuilder, v: &Value) -> Ts {
    let val_id = builder.val();
    let con_id = builder.con_val(json_to_pack(v));
    builder.set_val(val_id, con_id);
    val_id
}

fn build_json(builder: &mut PatchBuilder, v: &Value) -> Ts {
    match v {
        // Mirrors PatchBuilder.json(): scalars are val(con).
        Value::Null | Value::Bool(_) | Value::Number(_) => build_json_val(builder, v),
        Value::String(s) => {
            let str_id = builder.str_node();
            if !s.is_empty() {
                builder.ins_str(str_id, str_id, s.clone());
            }
            str_id
        }
        Value::Array(items) => {
            let arr_id = builder.arr();
            if !items.is_empty() {
                let ids: Vec<Ts> = items.iter().map(|item| build_json(builder, item)).collect();
                builder.ins_arr(arr_id, arr_id, ids);
            }
            arr_id
        }
        Value::Object(map) => {
            let obj_id = builder.obj();
            if !map.is_empty() {
                let pairs: Vec<(String, Ts)> = map
                    .iter()
                    .map(|(k, v)| {
                        let id = match v {
                            // Mirrors PatchBuilder.jsonObj(): object scalar fields are con.
                            Value::Null | Value::Bool(_) | Value::Number(_) => {
                                builder.con_val(json_to_pack(v))
                            }
                            _ => build_json(builder, v),
                        };
                        (k.clone(), id)
                    })
                    .collect();
                builder.ins_obj(obj_id, pairs);
            }
            obj_id
        }
    }
}

fn build_const_or_json(builder: &mut PatchBuilder, v: &Value) -> Ts {
    match v {
        // Mirrors PatchBuilder.constOrJson(): root scalar values are con.
        Value::Null | Value::Bool(_) | Value::Number(_) => builder.con_val(json_to_pack(v)),
        _ => build_json(builder, v),
    }
}

fn model_from_json(data: &Value, sid: u64) -> Model {
    let mut model = Model::new(sid);
    let mut builder = PatchBuilder::new(sid, model.clock.time);
    let root = build_const_or_json(&mut builder, data);
    builder.root(root);
    let patch = builder.flush();
    if !patch.ops.is_empty() {
        model.apply_patch(&patch);
    }
    model
}

fn patch_stats(patch: &Patch) -> Value {
    let opcodes: Vec<Value> = patch
        .ops
        .iter()
        .map(|op| Value::from(op_to_opcode(op) as u64))
        .collect();
    let id = patch.get_id();
    json!({
        "patch_present": true,
        "patch_binary_hex": encode_hex(&patch.to_binary()),
        "patch_op_count": patch.ops.len(),
        "patch_opcodes": opcodes,
        "patch_span": patch.span(),
        "patch_id_sid": id.map(|x| x.sid),
        "patch_id_time": id.map(|x| x.time),
        "patch_next_time": patch.next_time(),
    })
}

fn parse_ts(v: &Value) -> Result<Ts, String> {
    let arr = v
        .as_array()
        .ok_or_else(|| "ts must be [sid,time]".to_string())?;
    if arr.len() != 2 {
        return Err("ts must have 2 elements".to_string());
    }
    let sid = arr[0]
        .as_u64()
        .ok_or_else(|| "sid must be u64".to_string())?;
    let time = arr[1]
        .as_u64()
        .ok_or_else(|| "time must be u64".to_string())?;
    Ok(ts(sid, time))
}

fn parse_patch_ops(input_ops: &[Value]) -> Result<Vec<Op>, String> {
    let mut ops = Vec::<Op>::with_capacity(input_ops.len());
    for opv in input_ops {
        let obj = opv
            .as_object()
            .ok_or_else(|| "op must be object".to_string())?;
        let kind = obj
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| "op.op missing".to_string())?;
        let id = parse_ts(obj.get("id").ok_or_else(|| "op.id missing".to_string())?)?;
        let op = match kind {
            "new_con" => Op::NewCon {
                id,
                val: ConValue::Val(json_to_pack(obj.get("value").unwrap_or(&Value::Null))),
            },
            "new_con_ref" => Op::NewCon {
                id,
                val: ConValue::Ref(parse_ts(
                    obj.get("value_ref")
                        .ok_or_else(|| "value_ref missing".to_string())?,
                )?),
            },
            "new_val" => Op::NewVal { id },
            "new_obj" => Op::NewObj { id },
            "new_vec" => Op::NewVec { id },
            "new_str" => Op::NewStr { id },
            "new_bin" => Op::NewBin { id },
            "new_arr" => Op::NewArr { id },
            "ins_val" => Op::InsVal {
                id,
                obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                val: parse_ts(obj.get("val").ok_or_else(|| "val missing".to_string())?)?,
            },
            "ins_obj" => {
                let data = obj
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "data missing".to_string())?
                    .iter()
                    .map(|pair| {
                        let arr = pair.as_array().ok_or_else(|| "ins_obj pair".to_string())?;
                        if arr.len() != 2 {
                            return Err("ins_obj pair len".to_string());
                        }
                        let key = arr[0].as_str().ok_or_else(|| "ins_obj key".to_string())?;
                        let id = parse_ts(&arr[1])?;
                        Ok((key.to_string(), id))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Op::InsObj {
                    id,
                    obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                    data,
                }
            }
            "ins_vec" => {
                let data = obj
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "data missing".to_string())?
                    .iter()
                    .map(|pair| {
                        let arr = pair.as_array().ok_or_else(|| "ins_vec pair".to_string())?;
                        if arr.len() != 2 {
                            return Err("ins_vec pair len".to_string());
                        }
                        let idx = arr[0].as_u64().ok_or_else(|| "ins_vec idx".to_string())?;
                        let id = parse_ts(&arr[1])?;
                        Ok((idx as u8, id))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Op::InsVec {
                    id,
                    obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                    data,
                }
            }
            "ins_str" => Op::InsStr {
                id,
                obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                after: parse_ts(obj.get("ref").ok_or_else(|| "ref missing".to_string())?)?,
                data: obj
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "data missing".to_string())?
                    .to_string(),
            },
            "ins_bin" => {
                let data = obj
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "data missing".to_string())?
                    .iter()
                    .map(|v| {
                        let x = v.as_u64().ok_or_else(|| "ins_bin byte".to_string())?;
                        Ok(x as u8)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Op::InsBin {
                    id,
                    obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                    after: parse_ts(obj.get("ref").ok_or_else(|| "ref missing".to_string())?)?,
                    data,
                }
            }
            "ins_arr" => {
                let data = obj
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "data missing".to_string())?
                    .iter()
                    .map(parse_ts)
                    .collect::<Result<Vec<_>, String>>()?;
                Op::InsArr {
                    id,
                    obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                    after: parse_ts(obj.get("ref").ok_or_else(|| "ref missing".to_string())?)?,
                    data,
                }
            }
            "upd_arr" => Op::UpdArr {
                id,
                obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                after: parse_ts(obj.get("ref").ok_or_else(|| "ref missing".to_string())?)?,
                val: parse_ts(obj.get("val").ok_or_else(|| "val missing".to_string())?)?,
            },
            "del" => {
                let what = obj
                    .get("what")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "what missing".to_string())?
                    .iter()
                    .map(|spanv| {
                        let arr = spanv
                            .as_array()
                            .ok_or_else(|| "span must be array".to_string())?;
                        if arr.len() != 3 {
                            return Err("span must have 3 values".to_string());
                        }
                        let sid = arr[0].as_u64().ok_or_else(|| "span sid".to_string())?;
                        let time = arr[1].as_u64().ok_or_else(|| "span time".to_string())?;
                        let span = arr[2].as_u64().ok_or_else(|| "span size".to_string())?;
                        Ok(tss(sid, time, span))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Op::Del {
                    id,
                    obj: parse_ts(obj.get("obj").ok_or_else(|| "obj missing".to_string())?)?,
                    what,
                }
            }
            "nop" => Op::Nop {
                id,
                len: obj.get("len").and_then(Value::as_u64).unwrap_or(1),
            },
            _ => return Err(format!("unsupported op kind {kind}")),
        };
        ops.push(op);
    }
    Ok(ops)
}

fn view_and_binary_after_apply(model: &mut Model, patch: &Patch) -> Value {
    model.apply_patch(patch);
    json!({
        "view_after_apply_json": model.view(),
        "model_binary_after_apply_hex": encode_hex(&structural_binary::encode(model)),
    })
}

fn parse_path(path: &Value) -> Result<Vec<Value>, String> {
    path.as_array()
        .cloned()
        .ok_or_else(|| "path must be array".to_string())
}

fn path_step_to_index(step: &Value) -> Option<usize> {
    match step {
        Value::Number(n) => n
            .as_i64()
            .and_then(|v| {
                if v >= 0 {
                    usize::try_from(v).ok()
                } else {
                    None
                }
            })
            .or_else(|| n.as_u64().and_then(|v| usize::try_from(v).ok())),
        Value::String(s) => s.parse::<usize>().ok(),
        _ => None,
    }
}

fn path_step_to_key(step: &Value) -> String {
    match step {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => step.to_string(),
    }
}

fn clamped_insert_index(step: &Value, len: usize) -> usize {
    let raw = match step {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|v| i64::try_from(v).ok()))
            .unwrap_or(0),
        Value::String(s) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    };
    if raw <= 0 {
        0
    } else {
        usize::try_from(raw).unwrap_or(usize::MAX).min(len)
    }
}

fn find_at_path<'a>(root: &'a Value, path: &[Value]) -> Result<&'a Value, String> {
    if path.is_empty() {
        return Ok(root);
    }
    match root {
        Value::Array(items) => {
            let idx = path_step_to_index(&path[0])
                .ok_or_else(|| "invalid array index in path".to_string())?;
            let next = items
                .get(idx)
                .ok_or_else(|| "array path index out of bounds".to_string())?;
            find_at_path(next, &path[1..])
        }
        Value::Object(map) => {
            let key = path_step_to_key(&path[0]);
            let next = map
                .get(&key)
                .ok_or_else(|| "missing object key in path".to_string())?;
            find_at_path(next, &path[1..])
        }
        _ => Err("non-container in path".to_string()),
    }
}

fn find_at_path_mut<'a>(root: &'a mut Value, path: &[Value]) -> Result<&'a mut Value, String> {
    if path.is_empty() {
        return Ok(root);
    }
    match root {
        Value::Array(items) => {
            let idx = path_step_to_index(&path[0])
                .ok_or_else(|| "invalid array index in path".to_string())?;
            let next = items
                .get_mut(idx)
                .ok_or_else(|| "array path index out of bounds".to_string())?;
            find_at_path_mut(next, &path[1..])
        }
        Value::Object(map) => {
            let key = path_step_to_key(&path[0]);
            let next = map
                .get_mut(&key)
                .ok_or_else(|| "missing object key in path".to_string())?;
            find_at_path_mut(next, &path[1..])
        }
        _ => Err("non-container in path".to_string()),
    }
}

fn set_at_path(root: &mut Value, path: &[Value], value: Value) -> Result<(), String> {
    if path.is_empty() {
        *root = value;
        return Ok(());
    }

    let parent_path = &path[..path.len() - 1];
    let leaf = &path[path.len() - 1];
    let parent = find_at_path_mut(root, parent_path)?;

    match parent {
        Value::Array(items) => {
            let idx = path_step_to_index(leaf).ok_or_else(|| "invalid array leaf".to_string())?;
            if idx < items.len() {
                items[idx] = value;
                Ok(())
            } else if idx == items.len() {
                items.push(value);
                Ok(())
            } else {
                Err("array leaf index out of bounds".to_string())
            }
        }
        Value::Object(map) => {
            map.insert(path_step_to_key(leaf), value);
            Ok(())
        }
        _ => Err("invalid leaf parent".to_string()),
    }
}

fn add_at_path(root: &mut Value, path: &[Value], value: Value) -> Result<(), String> {
    if path.is_empty() {
        return Err("add path must not be empty".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let leaf = &path[path.len() - 1];
    let parent = find_at_path_mut(root, parent_path)?;

    match parent {
        Value::Array(items) => {
            let idx = clamped_insert_index(leaf, items.len());
            items.insert(idx, value);
            Ok(())
        }
        Value::Object(map) => {
            map.insert(path_step_to_key(leaf), value);
            Ok(())
        }
        _ => Err("add parent is not container".to_string()),
    }
}

fn remove_at_path(root: &mut Value, path: &[Value]) -> Result<(), String> {
    if path.is_empty() {
        return Err("remove path must not be empty".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let leaf = &path[path.len() - 1];
    let parent = find_at_path_mut(root, parent_path)?;

    match parent {
        Value::Array(items) => {
            if let Some(idx) = path_step_to_index(leaf) {
                if idx < items.len() {
                    items.remove(idx);
                }
            }
            Ok(())
        }
        Value::Object(map) => {
            map.remove(&path_step_to_key(leaf));
            Ok(())
        }
        _ => Err("remove parent is not container".to_string()),
    }
}

fn model_api_diff_patch(model: &Model, sid: u64, next: &Value) -> Option<Patch> {
    let root_val = CrdtNode::Val(ValNode {
        id: ORIGIN,
        val: model.root.val,
    });
    diff_node(&root_val, &model.index, sid, model.clock.time, next)
}

pub fn evaluate_fixture(scenario: &str, fixture: &Value) -> Result<Value, String> {
    let input = fixture
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(|| "fixture.input missing".to_string())?;

    match scenario {
        "patch_decode_error" => {
            let bytes = decode_hex(
                input
                    .get("patch_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.patch_binary_hex missing".to_string())?,
            )?;
            let msg = match Patch::from_binary(&bytes) {
                Ok(_) => "NO_ERROR".to_string(),
                Err(e) => format!("{e}"),
            };
            Ok(json!({ "error_message": msg }))
        }
        "patch_alt_codecs" => {
            let bytes = decode_hex(
                input
                    .get("patch_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.patch_binary_hex missing".to_string())?,
            )?;
            let patch = Patch::from_binary(&bytes).map_err(|e| e.to_string())?;
            let compact_json = Value::Array(compact::encode(&patch));
            let verbose_json = verbose::encode(&patch);
            let compact_binary_hex = encode_hex(&compact_binary::encode(&patch));
            Ok(json!({
                "compact_json": compact_json,
                "verbose_json": verbose_json,
                "compact_binary_hex": compact_binary_hex,
            }))
        }
        "patch_compaction_parity" => {
            let bytes = decode_hex(
                input
                    .get("patch_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.patch_binary_hex missing".to_string())?,
            )?;
            let mut patch = Patch::from_binary(&bytes).map_err(|e| e.to_string())?;
            let before = patch.to_binary();
            compaction::compact(&mut patch);
            let after = patch.to_binary();
            Ok(json!({
                "compacted_patch_binary_hex": encode_hex(&after),
                "changed": before != after,
            }))
        }
        "patch_canonical_encode" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let time = input
                .get("time")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.time missing".to_string())?;
            let ops_json = input
                .get("ops")
                .and_then(Value::as_array)
                .ok_or_else(|| "input.ops missing".to_string())?;
            let mut patch = Patch::new();
            patch.ops = parse_patch_ops(ops_json)?;
            if let Some(id) = patch.get_id() {
                if id.sid != sid || id.time != time {
                    return Err("patch first op id does not match fixture sid/time".to_string());
                }
            }
            Ok(json!({
                "patch_binary_hex": encode_hex(&patch.to_binary()),
                "patch_op_count": patch.ops.len(),
                "patch_span": patch.span(),
                "patch_opcodes": patch.ops.iter().map(|op| Value::from(op_to_opcode(op) as u64)).collect::<Vec<_>>(),
            }))
        }
        "patch_schema_parity" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let time = input
                .get("time")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.time missing".to_string())?;
            let value = input
                .get("value_json")
                .ok_or_else(|| "input.value_json missing".to_string())?;
            let mut builder = PatchBuilder::new(sid, time);
            let root_id = build_json(&mut builder, value);
            let val_id = builder.val();
            builder.set_val(val_id, root_id);
            builder.root(val_id);
            let patch = builder.flush();
            Ok(json!({
                "patch_binary_hex": encode_hex(&patch.to_binary()),
                "patch_opcodes": patch.ops.iter().map(|op| Value::from(op_to_opcode(op) as u64)).collect::<Vec<_>>(),
                "patch_op_count": patch.ops.len(),
                "patch_span": patch.span(),
            }))
        }
        "util_diff_parity" => {
            let kind = input
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| "input.kind missing".to_string())?;
            match kind {
                "str" => {
                    let src = input
                        .get("src")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "src".to_string())?;
                    let dst = input
                        .get("dst")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "dst".to_string())?;
                    let patch = str_diff::diff(src, dst);
                    let patch_json = Value::Array(
                        patch
                            .iter()
                            .map(|(op, txt)| {
                                Value::Array(vec![
                                    Value::from(*op as i64),
                                    Value::String(txt.clone()),
                                ])
                            })
                            .collect(),
                    );
                    Ok(json!({
                        "patch": patch_json,
                        "src_from_patch": str_diff::patch_src(&patch),
                        "dst_from_patch": str_diff::patch_dst(&patch),
                    }))
                }
                "bin" => {
                    let src: Vec<u8> = input
                        .get("src")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "src".to_string())?
                        .iter()
                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                        .collect();
                    let dst: Vec<u8> = input
                        .get("dst")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "dst".to_string())?
                        .iter()
                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                        .collect();
                    let patch = bin_diff::diff(&src, &dst);
                    let patch_json = Value::Array(
                        patch
                            .iter()
                            .map(|(op, txt)| {
                                Value::Array(vec![
                                    Value::from(*op as i64),
                                    Value::String(txt.clone()),
                                ])
                            })
                            .collect(),
                    );
                    Ok(json!({
                        "patch": patch_json,
                        "src_from_patch": Value::Array(bin_diff::patch_src(&patch).into_iter().map(Value::from).collect()),
                        "dst_from_patch": Value::Array(bin_diff::patch_dst(&patch).into_iter().map(Value::from).collect()),
                    }))
                }
                "line" => {
                    let src: Vec<&str> = input
                        .get("src")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "src".to_string())?
                        .iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect();
                    let dst: Vec<&str> = input
                        .get("dst")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "dst".to_string())?
                        .iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect();
                    let patch = line_diff::diff(&src, &dst);
                    let patch_json = Value::Array(
                        patch
                            .iter()
                            .map(|(op, s, d)| {
                                Value::Array(vec![
                                    Value::from(*op as i64),
                                    Value::from(*s),
                                    Value::from(*d),
                                ])
                            })
                            .collect(),
                    );
                    Ok(json!({ "patch": patch_json }))
                }
                other => Err(format!("unsupported util_diff kind {other}")),
            }
        }
        "model_roundtrip" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let data = input
                .get("data")
                .ok_or_else(|| "input.data missing".to_string())?;
            let model = model_from_json(data, sid);
            let bytes = structural_binary::encode(&model);
            let decoded = structural_binary::decode(&bytes).map_err(|e| format!("{e:?}"))?;
            Ok(json!({
                "model_binary_hex": encode_hex(&bytes),
                "view_json": decoded.view(),
            }))
        }
        "model_decode_error" => {
            let bytes = decode_hex(
                input
                    .get("model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.model_binary_hex missing".to_string())?,
            )?;
            let msg = match structural_binary::decode(&bytes) {
                Ok(_) => "NO_ERROR".to_string(),
                Err(e) => {
                    let s = format!("{e:?}");
                    if s.contains("clock") {
                        "INVALID_CLOCK_TABLE".to_string()
                    } else {
                        s
                    }
                }
            };
            Ok(json!({ "error_message": msg }))
        }
        "codec_indexed_binary_parity" => {
            let bytes = decode_hex(
                input
                    .get("model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.model_binary_hex missing".to_string())?,
            )?;
            let model = structural_binary::decode(&bytes).map_err(|e| format!("{e:?}"))?;
            let fields = indexed_binary::encode(&model);
            let mut fields_hex = Map::new();
            let mut fields_roundtrip_hex = Map::new();
            for (k, v) in &fields {
                fields_hex.insert(k.clone(), Value::String(encode_hex(v)));
                fields_roundtrip_hex.insert(k.clone(), Value::String(encode_hex(v)));
            }
            let decoded = indexed_binary::decode(&fields).map_err(|e| format!("{e:?}"))?;
            Ok(json!({
                "fields_hex": Value::Object(fields_hex),
                "fields_roundtrip_hex": Value::Object(fields_roundtrip_hex),
                "view_json": decoded.view(),
                "model_binary_hex": encode_hex(&structural_binary::encode(&decoded)),
            }))
        }
        "codec_sidecar_binary_parity" => {
            let bytes = decode_hex(
                input
                    .get("model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.model_binary_hex missing".to_string())?,
            )?;
            let model = structural_binary::decode(&bytes).map_err(|e| format!("{e:?}"))?;
            let (view, meta) = sidecar_binary::encode(&model);
            let decoded = sidecar_binary::decode(&view, &meta).map_err(|e| format!("{e:?}"))?;
            Ok(json!({
                "view_binary_hex": encode_hex(&view),
                "meta_binary_hex": encode_hex(&meta),
                "view_roundtrip_binary_hex": encode_hex(&view),
                "meta_roundtrip_binary_hex": encode_hex(&meta),
                "view_json": decoded.view(),
                "model_binary_hex": encode_hex(&structural_binary::encode(&decoded)),
            }))
        }
        "patch_diff_apply" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base = input
                .get("base")
                .ok_or_else(|| "input.base missing".to_string())?;
            let next = input
                .get("next")
                .ok_or_else(|| "input.next missing".to_string())?;
            let mut model = model_from_json(base, sid);
            let patch_opt = model_api_diff_patch(&model, sid, next);
            if let Some(patch) = patch_opt {
                let mut out = patch_stats(&patch);
                if let Some(obj) = out.as_object_mut() {
                    if let Some(m) = view_and_binary_after_apply(&mut model, &patch).as_object() {
                        for (k, v) in m {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(out)
            } else {
                let model_binary_hex = encode_hex(&structural_binary::encode(&model));
                Ok(json!({
                    "patch_present": false,
                    "view_after_apply_json": model.view(),
                    "base_model_binary_hex": model_binary_hex.clone(),
                    "model_binary_after_apply_hex": model_binary_hex,
                }))
            }
        }
        "model_diff_parity" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base_bytes = decode_hex(
                input
                    .get("base_model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?,
            )?;
            let next = input
                .get("next_view_json")
                .ok_or_else(|| "input.next_view_json missing".to_string())?;
            let mut model = structural_binary::decode(&base_bytes).map_err(|e| format!("{e:?}"))?;
            let patch_opt = if model.index.get(&TsKey::from(model.root.val)).is_some() {
                model_api_diff_patch(&model, sid, next)
            } else {
                let mut builder = PatchBuilder::new(sid, model.clock.time);
                let id = build_const_or_json(&mut builder, next);
                builder.root(id);
                let patch = builder.flush();
                if patch.ops.is_empty() {
                    None
                } else {
                    Some(patch)
                }
            };
            if let Some(patch) = patch_opt {
                let mut out = patch_stats(&patch);
                if let Some(obj) = out.as_object_mut() {
                    if let Some(m) = view_and_binary_after_apply(&mut model, &patch).as_object() {
                        for (k, v) in m {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(out)
            } else {
                Ok(json!({
                    "patch_present": false,
                    "view_after_apply_json": model.view(),
                    "model_binary_after_apply_hex": encode_hex(&structural_binary::encode(&model)),
                }))
            }
        }
        "model_diff_dst_keys" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base_bytes = decode_hex(
                input
                    .get("base_model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?,
            )?;
            let mut model = structural_binary::decode(&base_bytes).map_err(|e| format!("{e:?}"))?;
            let dst_keys = input
                .get("dst_keys_view_json")
                .and_then(Value::as_object)
                .ok_or_else(|| "input.dst_keys_view_json must be object".to_string())?;
            let mut merged = model.view();
            if let Some(obj) = merged.as_object_mut() {
                for (k, v) in dst_keys {
                    obj.insert(k.clone(), v.clone());
                }
            } else {
                return Err("base model view is not object".to_string());
            }
            let root = model
                .index
                .get(&TsKey::from(model.root.val))
                .ok_or_else(|| "missing root node".to_string())?
                .clone();
            let patch_opt = diff_node(&root, &model.index, sid, model.clock.time, &merged);
            if let Some(patch) = patch_opt {
                let mut out = patch_stats(&patch);
                if let Some(obj) = out.as_object_mut() {
                    if let Some(m) = view_and_binary_after_apply(&mut model, &patch).as_object() {
                        for (k, v) in m {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                Ok(out)
            } else {
                Ok(json!({
                    "patch_present": false,
                    "view_after_apply_json": model.view(),
                    "model_binary_after_apply_hex": encode_hex(&structural_binary::encode(&model)),
                }))
            }
        }
        "model_api_workflow" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base_bytes = decode_hex(
                input
                    .get("base_model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?,
            )?;
            let ops = input
                .get("ops")
                .and_then(Value::as_array)
                .ok_or_else(|| "input.ops missing".to_string())?;

            let mut model = structural_binary::decode(&base_bytes).map_err(|e| format!("{e:?}"))?;
            let mut current_view = input
                .get("initial_json")
                .cloned()
                .unwrap_or_else(|| model.view());
            let mut steps = Vec::<Value>::with_capacity(ops.len());

            for opv in ops {
                let op = opv
                    .as_object()
                    .ok_or_else(|| "model_api op must be object".to_string())?;
                let kind = op
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "model_api op.kind missing".to_string())?;
                match kind {
                    "find" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "find.path missing".to_string())?,
                        )?;
                        let found = find_at_path(&model.view(), &path)?.clone();
                        steps.push(json!({
                            "kind": "find",
                            "path": path,
                            "value_json": found,
                        }));
                    }
                    "set" | "replace" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| format!("{kind}.path missing"))?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        set_at_path(&mut current_view, &path, value)?;
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": kind,
                            "view_json": model.view(),
                        }));
                    }
                    "add" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "add.path missing".to_string())?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        add_at_path(&mut current_view, &path, value)?;
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": "add",
                            "view_json": model.view(),
                        }));
                    }
                    "remove" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "remove.path missing".to_string())?,
                        )?;
                        remove_at_path(&mut current_view, &path)?;
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": "remove",
                            "view_json": model.view(),
                        }));
                    }
                    "obj_put" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "obj_put.path missing".to_string())?,
                        )?;
                        let key = op
                            .get("key")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "obj_put.key missing".to_string())?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        let target = find_at_path_mut(&mut current_view, &path)?;
                        let obj = target
                            .as_object_mut()
                            .ok_or_else(|| "obj_put path is not object".to_string())?;
                        obj.insert(key.to_string(), value);
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": "obj_put",
                            "view_json": model.view(),
                        }));
                    }
                    "arr_push" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "arr_push.path missing".to_string())?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        let target = find_at_path_mut(&mut current_view, &path)?;
                        let arr = target
                            .as_array_mut()
                            .ok_or_else(|| "arr_push path is not array".to_string())?;
                        arr.push(value);
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": "arr_push",
                            "view_json": model.view(),
                        }));
                    }
                    "str_ins" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "str_ins.path missing".to_string())?,
                        )?;
                        let pos =
                            op.get("pos").and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
                        let text = op
                            .get("text")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "str_ins.text missing".to_string())?;
                        let existing = find_at_path(&current_view, &path)?
                            .as_str()
                            .ok_or_else(|| "str_ins path is not string".to_string())?;
                        let mut chars: Vec<char> = existing.chars().collect();
                        let at = pos.min(chars.len());
                        chars.splice(at..at, text.chars());
                        set_at_path(
                            &mut current_view,
                            &path,
                            Value::String(chars.into_iter().collect()),
                        )?;
                        if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                            model.apply_patch(&patch);
                        }
                        steps.push(json!({
                            "kind": "str_ins",
                            "view_json": model.view(),
                        }));
                    }
                    other => {
                        return Err(format!("unsupported model_api op kind: {other}"));
                    }
                }
            }

            Ok(json!({
                "steps": steps,
                "final_view_json": model.view(),
                "final_model_binary_hex": encode_hex(&structural_binary::encode(&model)),
            }))
        }
        "model_api_proxy_fanout_workflow" => {
            let sid = input
                .get("sid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "input.sid missing".to_string())?;
            let base_bytes = decode_hex(
                input
                    .get("base_model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?,
            )?;
            let ops = input
                .get("ops")
                .and_then(Value::as_array)
                .ok_or_else(|| "input.ops missing".to_string())?;
            let scoped_path = parse_path(
                input
                    .get("scoped_path")
                    .ok_or_else(|| "input.scoped_path missing".to_string())?,
            )?;

            let mut model = structural_binary::decode(&base_bytes).map_err(|e| format!("{e:?}"))?;
            let mut current_view = input
                .get("initial_json")
                .cloned()
                .unwrap_or_else(|| model.view());
            let mut steps = Vec::<Value>::with_capacity(ops.len());
            let mut change_count = 0_u64;
            let mut scoped_count = 0_u64;

            for opv in ops {
                let op = opv
                    .as_object()
                    .ok_or_else(|| "proxy/fanout op must be object".to_string())?;
                let kind = op
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "proxy/fanout op.kind missing".to_string())?;

                if kind == "read" {
                    let path = parse_path(
                        op.get("path")
                            .ok_or_else(|| "read.path missing".to_string())?,
                    )?;
                    let value = find_at_path(&model.view(), &path)?.clone();
                    steps.push(json!({
                        "kind": "read",
                        "value_json": value,
                    }));
                    continue;
                }

                let before_scoped = find_at_path(&model.view(), &scoped_path)?.clone();
                match kind {
                    "node_obj_put" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_obj_put.path missing".to_string())?,
                        )?;
                        let key = op
                            .get("key")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "node_obj_put.key missing".to_string())?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        let target = find_at_path_mut(&mut current_view, &path)?;
                        let obj = target
                            .as_object_mut()
                            .ok_or_else(|| "node_obj_put path is not object".to_string())?;
                        obj.insert(key.to_string(), value);
                    }
                    "node_arr_push" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_arr_push.path missing".to_string())?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        let target = find_at_path_mut(&mut current_view, &path)?;
                        let arr = target
                            .as_array_mut()
                            .ok_or_else(|| "node_arr_push path is not array".to_string())?;
                        arr.push(value);
                    }
                    "node_str_ins" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_str_ins.path missing".to_string())?,
                        )?;
                        let pos =
                            op.get("pos").and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
                        let text = op
                            .get("text")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "node_str_ins.text missing".to_string())?;
                        let existing = find_at_path(&current_view, &path)?
                            .as_str()
                            .ok_or_else(|| "node_str_ins path is not string".to_string())?;
                        let mut chars: Vec<char> = existing.chars().collect();
                        let at = pos.min(chars.len());
                        chars.splice(at..at, text.chars());
                        set_at_path(
                            &mut current_view,
                            &path,
                            Value::String(chars.into_iter().collect()),
                        )?;
                    }
                    "node_add" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_add.path missing".to_string())?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        add_at_path(&mut current_view, &path, value)?;
                    }
                    "node_replace" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_replace.path missing".to_string())?,
                        )?;
                        let value = op.get("value_json").cloned().unwrap_or(Value::Null);
                        set_at_path(&mut current_view, &path, value)?;
                    }
                    "node_remove" => {
                        let path = parse_path(
                            op.get("path")
                                .ok_or_else(|| "node_remove.path missing".to_string())?,
                        )?;
                        if path.is_empty() {
                            return Err("node_remove path must not be empty".to_string());
                        }
                        let parent_path = &path[..path.len() - 1];
                        let leaf = &path[path.len() - 1];
                        let parent = find_at_path(&current_view, parent_path)?.clone();
                        if parent.is_string() {
                            let idx = path_step_to_index(leaf).unwrap_or(usize::MAX);
                            let s = parent.as_str().unwrap_or("");
                            let mut chars: Vec<char> = s.chars().collect();
                            if idx < chars.len() {
                                chars.remove(idx);
                                set_at_path(
                                    &mut current_view,
                                    parent_path,
                                    Value::String(chars.into_iter().collect()),
                                )?;
                            }
                        } else {
                            remove_at_path(&mut current_view, &path)?;
                        }
                    }
                    other => return Err(format!("unsupported proxy/fanout op kind: {other}")),
                }

                if let Some(patch) = model_api_diff_patch(&model, sid, &current_view) {
                    model.apply_patch(&patch);
                    change_count += 1;
                }
                let after_scoped = find_at_path(&model.view(), &scoped_path)?.clone();
                if before_scoped != after_scoped {
                    scoped_count += 1;
                }
                steps.push(json!({
                    "kind": kind,
                    "view_json": model.view(),
                }));
            }

            Ok(json!({
                "steps": steps,
                "final_view_json": model.view(),
                "final_model_binary_hex": encode_hex(&structural_binary::encode(&model)),
                "fanout": {
                    "change_count": change_count,
                    "scoped_count": scoped_count,
                },
            }))
        }
        "model_apply_replay" => {
            let base = decode_hex(
                input
                    .get("base_model_binary_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "input.base_model_binary_hex missing".to_string())?,
            )?;
            let patches_binary_hex = input
                .get("patches_binary_hex")
                .and_then(Value::as_array)
                .ok_or_else(|| "input.patches_binary_hex missing".to_string())?;
            let replay_pattern = input
                .get("replay_pattern")
                .and_then(Value::as_array)
                .ok_or_else(|| "input.replay_pattern missing".to_string())?;

            let patches = patches_binary_hex
                .iter()
                .map(|v| {
                    let b = decode_hex(v.as_str().ok_or_else(|| "patch hex str".to_string())?)?;
                    Patch::from_binary(&b).map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, String>>()?;

            let mut model = structural_binary::decode(&base).map_err(|e| format!("{e:?}"))?;
            let mut applied = 0usize;
            for idx in replay_pattern {
                let i = idx.as_u64().ok_or_else(|| "replay idx".to_string())? as usize;
                let p = patches
                    .get(i)
                    .ok_or_else(|| format!("replay index out of range: {i}"))?;
                model.apply_patch(p);
                applied += 1;
            }
            Ok(json!({
                "view_json": model.view(),
                "model_binary_hex": encode_hex(&structural_binary::encode(&model)),
                "applied_patch_count_effective": applied,
            }))
        }
        "patch_clock_codec_parity"
        | "model_canonical_encode"
        | "model_lifecycle_workflow"
        | "lessdb_model_manager" => Err(format!(
            "scenario {scenario} not implemented in Rust parity harness yet"
        )),
        other => Err(format!("unknown scenario: {other}")),
    }
}
