//! Structural verbose JSON codec.
//!
//! Mirrors:
//! - `structural/verbose/Encoder.ts`
//! - `structural/verbose/Decoder.ts`
//! - `structural/verbose/types.ts`
//!
//! Wire format: a JSON object with full field names.
//!
//! ```json
//! {
//!   "time": <number | [sid, time, ...]>,
//!   "root": { "type": "val", "id": ..., "value": <node> }
//! }
//! ```

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};

use crate::json_crdt::constants::UNDEFINED_TS;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, ObjNode, StrNode, TsKey, ValNode, VecNode,
};
use crate::json_crdt_patch::clock::{ts as mk_ts, ClockVector, Ts};
use crate::json_crdt_patch::enums::SESSION;
use crate::json_crdt_patch::operations::ConValue;
use json_joy_json_pack::PackValue;

// ── Encode ──────────────────────────────────────────────────────────────────

/// Encode a [`Model`] to the verbose JSON format.
pub fn encode(model: &Model) -> Value {
    let is_server = model.clock.sid == SESSION::SERVER;
    let time: Value = if is_server {
        json!(model.clock.time)
    } else {
        encode_clock(&model.clock)
    };

    let root = encode_val_root(model);
    json!({ "time": time, "root": root })
}

fn encode_clock(clock: &ClockVector) -> Value {
    let mut entries = Vec::new();
    // Local session first
    let local_ts = json!([clock.sid, clock.time]);
    entries.push(local_ts);
    // Peer sessions
    for peer in clock.peers.values() {
        entries.push(json!([peer.sid, peer.time]));
    }
    Value::Array(entries)
}

fn encode_ts(stamp: Ts) -> Value {
    if stamp.sid == SESSION::SERVER {
        json!(stamp.time)
    } else {
        json!([stamp.sid, stamp.time])
    }
}

fn encode_val_root(model: &Model) -> Value {
    let root_ts = model.root.val;
    let id = encode_ts(UNDEFINED_TS);
    if root_ts == UNDEFINED_TS || root_ts.time == 0 {
        // empty root: val node pointing to undefined
        json!({
            "type": "val",
            "id": id,
            "value": encode_con_undefined()
        })
    } else {
        let child_node = model.index.get(&TsKey::from(root_ts));
        let value = match child_node {
            Some(n) => encode_node(model, n),
            None => encode_con_undefined(),
        };
        json!({
            "type": "val",
            "id": encode_ts(root_ts),
            "value": value
        })
    }
}

fn encode_con_undefined() -> Value {
    json!({ "type": "con", "id": encode_ts(UNDEFINED_TS) })
}

fn encode_node(model: &Model, node: &CrdtNode) -> Value {
    match node {
        CrdtNode::Con(n) => encode_con(n),
        CrdtNode::Val(n) => encode_val(model, n),
        CrdtNode::Obj(n) => encode_obj(model, n),
        CrdtNode::Vec(n) => encode_vec(model, n),
        CrdtNode::Str(n) => encode_str(n),
        CrdtNode::Bin(n) => encode_bin(n),
        CrdtNode::Arr(n) => encode_arr(model, n),
    }
}

fn encode_con(node: &ConNode) -> Value {
    let id = encode_ts(node.id);
    match &node.val {
        ConValue::Ref(ref_ts) => {
            json!({
                "type": "con",
                "id": id,
                "timestamp": true,
                "value": encode_ts(*ref_ts)
            })
        }
        ConValue::Val(pv) => match pv {
            PackValue::Undefined => {
                json!({ "type": "con", "id": id })
            }
            _ => {
                let v = serde_json::Value::from(pv.clone());
                json!({ "type": "con", "id": id, "value": v })
            }
        },
    }
}

fn encode_val(model: &Model, node: &ValNode) -> Value {
    let id = encode_ts(node.id);
    let child_ts = node.val;
    let value = match model.index.get(&TsKey::from(child_ts)) {
        Some(n) => encode_node(model, n),
        None => encode_con_undefined(),
    };
    json!({ "type": "val", "id": id, "value": value })
}

fn encode_obj(model: &Model, node: &ObjNode) -> Value {
    let id = encode_ts(node.id);
    let mut map = serde_json::Map::new();
    let mut sorted_keys: Vec<&String> = node.keys.keys().collect();
    sorted_keys.sort();
    for key in &sorted_keys {
        let child_ts = node.keys[key.as_str()];
        if let Some(child) = model.index.get(&TsKey::from(child_ts)) {
            map.insert((*key).clone(), encode_node(model, child));
        }
    }
    json!({ "type": "obj", "id": id, "map": map })
}

fn encode_vec(model: &Model, node: &VecNode) -> Value {
    let id = encode_ts(node.id);
    let elements: Vec<Value> = node
        .elements
        .iter()
        .map(|e| match e {
            None => Value::Null,
            Some(child_ts) => match model.index.get(&TsKey::from(*child_ts)) {
                Some(child) => encode_node(model, child),
                None => Value::Null,
            },
        })
        .collect();
    json!({ "type": "vec", "id": id, "map": elements })
}

fn encode_str(node: &StrNode) -> Value {
    let id = encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = encode_ts(chunk.id);
            if chunk.deleted {
                json!({ "id": chunk_id, "span": chunk.span })
            } else {
                let data = chunk.data.as_deref().unwrap_or("");
                json!({ "id": chunk_id, "value": data })
            }
        })
        .collect();
    json!({ "type": "str", "id": id, "chunks": chunks })
}

fn encode_bin(node: &BinNode) -> Value {
    let id = encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = encode_ts(chunk.id);
            if chunk.deleted {
                json!({ "id": chunk_id, "span": chunk.span })
            } else {
                let data = chunk.data.as_deref().unwrap_or(&[]);
                let b64 = B64.encode(data);
                json!({ "id": chunk_id, "value": b64 })
            }
        })
        .collect();
    json!({ "type": "bin", "id": id, "chunks": chunks })
}

fn encode_arr(model: &Model, node: &ArrNode) -> Value {
    let id = encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = encode_ts(chunk.id);
            if chunk.deleted {
                json!({ "id": chunk_id, "span": chunk.span })
            } else {
                let ids = chunk.data.as_deref().unwrap_or(&[]);
                let values: Vec<Value> = ids
                    .iter()
                    .filter_map(|id| {
                        model
                            .index
                            .get(&TsKey::from(*id))
                            .map(|n| encode_node(model, n))
                    })
                    .collect();
                json!({ "id": chunk_id, "value": values })
            }
        })
        .collect();
    json!({ "type": "arr", "id": id, "chunks": chunks })
}

// ── Decode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during verbose decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("unexpected format: {0}")]
    Format(String),
    #[error("unknown node type: {0}")]
    UnknownNodeType(String),
    #[error("missing field: {0}")]
    MissingField(String),
}

/// Decode a verbose JSON document back into a [`Model`].
pub fn decode(data: &Value) -> Result<Model, DecodeError> {
    let obj = data
        .as_object()
        .ok_or_else(|| DecodeError::Format("expected object".into()))?;

    let time_val = obj
        .get("time")
        .ok_or_else(|| DecodeError::MissingField("time".into()))?;
    let root_val = obj
        .get("root")
        .ok_or_else(|| DecodeError::MissingField("root".into()))?;

    let is_server = time_val.is_number();
    let mut model = if is_server {
        let server_time = time_val
            .as_u64()
            .ok_or_else(|| DecodeError::Format("server time must be u64".into()))?;
        Model::new_server(server_time)
    } else {
        let timestamps = time_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("logical clock must be array".into()))?;
        let clock = decode_clock(timestamps)?;
        Model::new_from_clock(clock)
    };

    decode_root(root_val, &mut model)?;
    Ok(model)
}

fn decode_clock(timestamps: &[Value]) -> Result<ClockVector, DecodeError> {
    if timestamps.is_empty() {
        return Err(DecodeError::Format("clock table is empty".into()));
    }
    let first = &timestamps[0];
    let first_arr = first
        .as_array()
        .ok_or_else(|| DecodeError::Format("clock entry must be array".into()))?;
    if first_arr.len() < 2 {
        return Err(DecodeError::Format("clock entry too short".into()));
    }
    let sid = first_arr[0].as_u64().unwrap_or(0);
    let time = first_arr[1].as_u64().unwrap_or(0);
    let mut clock = ClockVector::new(sid, time);

    for stamp_val in &timestamps[1..] {
        let stamp_arr = stamp_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("clock entry must be array".into()))?;
        if stamp_arr.len() >= 2 {
            let peer_sid = stamp_arr[0].as_u64().unwrap_or(0);
            let peer_time = stamp_arr[1].as_u64().unwrap_or(0);
            clock.observe(mk_ts(peer_sid, peer_time), 1);
        }
    }
    Ok(clock)
}

fn decode_ts(val: &Value) -> Result<Ts, DecodeError> {
    if let Some(t) = val.as_u64() {
        return Ok(mk_ts(SESSION::SERVER, t));
    }
    if let Some(arr) = val.as_array() {
        if arr.len() >= 2 {
            let sid = arr[0].as_u64().unwrap_or(0);
            let time = arr[1].as_u64().unwrap_or(0);
            return Ok(mk_ts(sid, time));
        }
    }
    Err(DecodeError::Format(format!("invalid timestamp: {}", val)))
}

fn decode_root(val: &Value, model: &mut Model) -> Result<(), DecodeError> {
    let obj = val
        .as_object()
        .ok_or_else(|| DecodeError::Format("root must be object".into()))?;

    // Root is always a "val" node
    let value_val = obj
        .get("value")
        .ok_or_else(|| DecodeError::MissingField("root.value".into()))?;
    let child_id = decode_node(value_val, model)?;
    model.root.val = child_id;
    Ok(())
}

fn decode_node(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val
        .as_object()
        .ok_or_else(|| DecodeError::Format("node must be object".into()))?;

    let type_str = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DecodeError::MissingField("type".into()))?;

    match type_str {
        "con" => decode_con(val, model),
        "val" => decode_val(val, model),
        "obj" => decode_obj(val, model),
        "vec" => decode_vec(val, model),
        "str" => decode_str(val, model),
        "bin" => decode_bin(val, model),
        "arr" => decode_arr(val, model),
        other => Err(DecodeError::UnknownNodeType(other.into())),
    }
}

fn decode_con(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("con.id".into()))?,
    )?;

    let con_val = if obj
        .get("timestamp")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let ts_val = obj
            .get("value")
            .ok_or_else(|| DecodeError::MissingField("con.value (timestamp)".into()))?;
        let ref_ts = decode_ts(ts_val)?;
        ConValue::Ref(ref_ts)
    } else {
        match obj.get("value") {
            None => ConValue::Val(PackValue::Undefined),
            Some(v) => ConValue::Val(PackValue::from(v.clone())),
        }
    };

    use crate::json_crdt::nodes::ConNode;
    let node = CrdtNode::Con(ConNode::new(id, con_val));
    model.index.insert(TsKey::from(id), node);
    Ok(id)
}

fn decode_val(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("val.id".into()))?,
    )?;

    let child_id = match obj.get("value") {
        Some(v) => decode_node(v, model)?,
        None => {
            // Create a stub undefined con node
            let stub_id = id; // reuse parent id as stub — upstream would error; just use id
            let con = CrdtNode::Con(crate::json_crdt::nodes::ConNode::new(
                stub_id,
                ConValue::Val(PackValue::Undefined),
            ));
            model.index.insert(TsKey::from(stub_id), con);
            stub_id
        }
    };

    use crate::json_crdt::nodes::ValNode;
    let mut node = ValNode::new(id);
    node.val = child_id;
    model.index.insert(TsKey::from(id), CrdtNode::Val(node));
    Ok(id)
}

fn decode_obj(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("obj.id".into()))?,
    )?;

    let map = obj
        .get("map")
        .and_then(|v| v.as_object())
        .ok_or_else(|| DecodeError::MissingField("obj.map".into()))?;

    use crate::json_crdt::nodes::ObjNode;
    let mut node = ObjNode::new(id);
    let entries: Vec<(String, Value)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    for (key, child_val) in entries {
        let child_id = decode_node(&child_val, model)?;
        node.keys.insert(key, child_id);
    }
    model.index.insert(TsKey::from(id), CrdtNode::Obj(node));
    Ok(id)
}

fn decode_vec(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("vec.id".into()))?,
    )?;

    let map_arr = obj
        .get("map")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DecodeError::MissingField("vec.map".into()))?
        .clone();

    use crate::json_crdt::nodes::VecNode;
    let mut node = VecNode::new(id);
    for elem in &map_arr {
        if elem.is_null() {
            node.elements.push(None);
        } else {
            let child_id = decode_node(elem, model)?;
            node.elements.push(Some(child_id));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Vec(node));
    Ok(id)
}

fn decode_str(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("str.id".into()))?,
    )?;

    let chunks_arr = obj
        .get("chunks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DecodeError::MissingField("str.chunks".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::StrNode;

    let mut node = StrNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_obj = chunk_val
            .as_object()
            .ok_or_else(|| DecodeError::Format("str chunk must be object".into()))?;
        let chunk_id = decode_ts(
            chunk_obj
                .get("id")
                .ok_or_else(|| DecodeError::MissingField("chunk.id".into()))?,
        )?;
        if let Some(span) = chunk_obj.get("span").and_then(|v| v.as_u64()) {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(s) = chunk_obj.get("value").and_then(|v| v.as_str()) {
            let span = s.chars().count() as u64;
            node.rga
                .push_chunk(Chunk::new(chunk_id, span, s.to_string()));
        } else {
            return Err(DecodeError::Format(
                "str chunk must have span or value".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Str(node));
    Ok(id)
}

fn decode_bin(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("bin.id".into()))?,
    )?;

    let chunks_arr = obj
        .get("chunks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DecodeError::MissingField("bin.chunks".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::BinNode;

    let mut node = BinNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_obj = chunk_val
            .as_object()
            .ok_or_else(|| DecodeError::Format("bin chunk must be object".into()))?;
        let chunk_id = decode_ts(
            chunk_obj
                .get("id")
                .ok_or_else(|| DecodeError::MissingField("chunk.id".into()))?,
        )?;
        if let Some(span) = chunk_obj.get("span").and_then(|v| v.as_u64()) {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(b64) = chunk_obj.get("value").and_then(|v| v.as_str()) {
            let data = B64
                .decode(b64)
                .map_err(|e| DecodeError::Format(format!("base64 decode error: {}", e)))?;
            let span = data.len() as u64;
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        } else {
            return Err(DecodeError::Format(
                "bin chunk must have span or value".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Bin(node));
    Ok(id)
}

fn decode_arr(val: &Value, model: &mut Model) -> Result<Ts, DecodeError> {
    let obj = val.as_object().unwrap();
    let id = decode_ts(
        obj.get("id")
            .ok_or_else(|| DecodeError::MissingField("arr.id".into()))?,
    )?;

    let chunks_arr = obj
        .get("chunks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DecodeError::MissingField("arr.chunks".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::ArrNode;

    let mut node = ArrNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_obj = chunk_val
            .as_object()
            .ok_or_else(|| DecodeError::Format("arr chunk must be object".into()))?;
        let chunk_id = decode_ts(
            chunk_obj
                .get("id")
                .ok_or_else(|| DecodeError::MissingField("chunk.id".into()))?,
        )?;
        if let Some(span) = chunk_obj.get("span").and_then(|v| v.as_u64()) {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(values) = chunk_obj.get("value").and_then(|v| v.as_array()) {
            let values = values.clone();
            let mut ids = Vec::new();
            for child_val in &values {
                let child_id = decode_node(child_val, model)?;
                ids.push(child_id);
            }
            let span = ids.len() as u64;
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        } else {
            return Err(DecodeError::Format(
                "arr chunk must have span or value".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Arr(node));
    Ok(id)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 {
        654321
    }

    #[test]
    fn encode_empty_model() {
        let model = Model::new(sid());
        let encoded = encode(&model);
        assert!(encoded.get("time").is_some());
        assert!(encoded.get("root").is_some());
    }

    #[test]
    fn roundtrip_simple_string() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: crate::json_crdt::constants::ORIGIN,
            data: "test".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });

        let view = model.view();
        let encoded = encode(&model);
        let decoded = decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded.view(), view);
    }

    #[test]
    fn roundtrip_con_number() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(100)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });

        let view = model.view();
        let encoded = encode(&model);
        let decoded = decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded.view(), view);
    }
}
