//! Structural compact JSON codec.
//!
//! Mirrors:
//! - `structural/compact/Encoder.ts`
//! - `structural/compact/Decoder.ts`
//! - `structural/compact/types.ts`
//!
//! Wire format:
//! ```json
//! [clock_table_or_server_time, root_node_or_0]
//! ```
//!
//! `clock_table_or_server_time`:
//! - A plain integer → server-clock mode (value is the server time).
//! - An array of numbers `[sid, time, sid, time, ...]` → logical-clock mode.
//!
//! Timestamps within node data:
//! - Server mode: a plain integer `server_time - ts.time`.
//! - System session (sid=0): `[sid, time]` (literal).
//! - Logical session: `[-session_index, time_diff]` (negative first element).

use serde_json::{json, Value};

use crate::json_crdt::constants::UNDEFINED_TS;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, IndexExt, NodeIndex, ObjNode, RootNode, StrNode, TsKey,
    ValNode, VecNode,
};
use crate::json_crdt_patch::clock::{ts as mk_ts, ClockVector, Ts};
use crate::json_crdt_patch::codec::clock::{ClockDecoder, ClockEncoder};
use crate::json_crdt_patch::enums::{JsonCrdtDataType, SESSION};
use crate::json_crdt_patch::operations::ConValue;
use json_joy_json_pack::PackValue;

// ── CRDT data-type constants ────────────────────────────────────────────────

const CON: u8 = JsonCrdtDataType::Con as u8;
const VAL: u8 = JsonCrdtDataType::Val as u8;
const OBJ: u8 = JsonCrdtDataType::Obj as u8;
const VEC: u8 = JsonCrdtDataType::Vec as u8;
const STR: u8 = JsonCrdtDataType::Str as u8;
const BIN: u8 = JsonCrdtDataType::Bin as u8;
const ARR: u8 = JsonCrdtDataType::Arr as u8;

// ── Encode ──────────────────────────────────────────────────────────────────

/// Encode a [`Model`] to the compact JSON format.
///
/// Returns a 2-element JSON array `[clock, root]`.
pub fn encode(model: &Model) -> Value {
    let is_server = model.clock.sid == SESSION::SERVER;
    if is_server {
        let server_time = model.clock.time;
        let root = encode_root(model, &mut EncodeState::Server(server_time));
        json!([server_time, root])
    } else {
        let mut enc = ClockEncoder::new();
        enc.reset(&model.clock);
        let mut state = EncodeState::Logical(enc);
        let root = encode_root(model, &mut state);
        let clock_table = match &state {
            EncodeState::Logical(enc) => {
                let flat = enc.to_json();
                Value::Array(flat.into_iter().map(|n| json!(n)).collect())
            }
            _ => unreachable!(),
        };
        json!([clock_table, root])
    }
}

enum EncodeState {
    Server(u64),
    Logical(ClockEncoder),
}

impl EncodeState {
    fn encode_ts(&mut self, stamp: Ts) -> Value {
        match self {
            EncodeState::Server(server_time) => {
                if stamp.sid == SESSION::SYSTEM {
                    // System session: encode as [sid, time]
                    json!([stamp.sid, stamp.time])
                } else {
                    json!(*server_time - stamp.time)
                }
            }
            EncodeState::Logical(enc) => {
                if stamp.sid == SESSION::SYSTEM {
                    // System session: encode as [sid, time] (positive)
                    json!([stamp.sid, stamp.time])
                } else {
                    match enc.append(stamp) {
                        Ok(rel) => {
                            // Encode as [-session_index, time_diff]
                            let neg_idx = -(rel.session_index as i64);
                            json!([neg_idx, rel.time_diff])
                        }
                        Err(_) => json!([stamp.sid, stamp.time]),
                    }
                }
            }
        }
    }
}

fn encode_root(model: &Model, state: &mut EncodeState) -> Value {
    let root_ts = model.root.val;
    if root_ts == UNDEFINED_TS || root_ts.time == 0 {
        json!(0)
    } else {
        match model.index.get(&TsKey::from(root_ts)) {
            Some(node) => encode_node(model, node, state),
            None => json!(0),
        }
    }
}

fn encode_node(model: &Model, node: &CrdtNode, state: &mut EncodeState) -> Value {
    match node {
        CrdtNode::Con(n) => encode_con(n, state),
        CrdtNode::Val(n) => encode_val(model, n, state),
        CrdtNode::Obj(n) => encode_obj(model, n, state),
        CrdtNode::Vec(n) => encode_vec(model, n, state),
        CrdtNode::Str(n) => encode_str(n, state),
        CrdtNode::Bin(n) => encode_bin(n, state),
        CrdtNode::Arr(n) => encode_arr(model, n, state),
    }
}

fn encode_con(node: &ConNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    match &node.val {
        ConValue::Ref(ref_ts) => {
            // Special: [CON, id, 0, encoded_ref_ts]
            let special = state.encode_ts(*ref_ts);
            json!([CON, id, 0, special])
        }
        ConValue::Val(pv) => {
            match pv {
                PackValue::Undefined => {
                    // undefined: [CON, id, 0, 0]
                    json!([CON, id, 0, 0])
                }
                _ => {
                    let v = serde_json::Value::from(pv.clone());
                    json!([CON, id, v])
                }
            }
        }
    }
}

fn encode_val(model: &Model, node: &ValNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let child_ts = node.val;
    let child_node = model.index.get(&TsKey::from(child_ts));
    let child = match child_node {
        Some(n) => encode_node(model, n, state),
        None => json!(null),
    };
    json!([VAL, id, child])
}

fn encode_obj(model: &Model, node: &ObjNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let mut map = serde_json::Map::new();
    let mut sorted_keys: Vec<&String> = node.keys.keys().collect();
    sorted_keys.sort();
    for key in &sorted_keys {
        let child_ts = node.keys[key.as_str()];
        if let Some(child) = model.index.get(&TsKey::from(child_ts)) {
            map.insert((*key).clone(), encode_node(model, child, state));
        }
    }
    json!([OBJ, id, Value::Object(map)])
}

fn encode_vec(model: &Model, node: &VecNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let elements: Vec<Value> = node
        .elements
        .iter()
        .map(|e| match e {
            None => json!(0),
            Some(child_ts) => match model.index.get(&TsKey::from(*child_ts)) {
                Some(child) => encode_node(model, child, state),
                None => json!(0),
            },
        })
        .collect();
    json!([VEC, id, elements])
}

fn encode_str(node: &StrNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = state.encode_ts(chunk.id);
            if chunk.deleted {
                json!([chunk_id, chunk.span])
            } else {
                let data = chunk.data.as_deref().unwrap_or("");
                json!([chunk_id, data])
            }
        })
        .collect();
    json!([STR, id, chunks])
}

fn encode_bin(node: &BinNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = state.encode_ts(chunk.id);
            if chunk.deleted {
                json!([chunk_id, chunk.span])
            } else {
                let data = chunk.data.as_deref().unwrap_or(&[]);
                // Encode binary as array of byte values (JSON-safe)
                let bytes: Vec<Value> = data.iter().map(|&b| json!(b)).collect();
                json!([chunk_id, bytes])
            }
        })
        .collect();
    json!([BIN, id, chunks])
}

fn encode_arr(model: &Model, node: &ArrNode, state: &mut EncodeState) -> Value {
    let id = state.encode_ts(node.id);
    let chunks: Vec<Value> = node
        .rga
        .iter()
        .map(|chunk| {
            let chunk_id = state.encode_ts(chunk.id);
            if chunk.deleted {
                json!([chunk_id, chunk.span])
            } else {
                let node_ids = chunk.data.as_ref().map(|ids| ids.as_slice()).unwrap_or(&[]);
                let nodes: Vec<Value> = node_ids
                    .iter()
                    .filter_map(|id| {
                        model
                            .index
                            .get(&TsKey::from(*id))
                            .map(|n| encode_node(model, n, state))
                    })
                    .collect();
                json!([chunk_id, nodes])
            }
        })
        .collect();
    json!([ARR, id, chunks])
}

// ── Decode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during compact decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("unexpected format: {0}")]
    Format(String),
    #[error("unknown node type: {0}")]
    UnknownNodeType(u64),
    #[error("missing field")]
    MissingField,
    #[error("clock decoder not initialised")]
    NoClockDecoder,
    #[error("invalid session index")]
    InvalidSessionIndex,
}

/// Decode a compact JSON document back into a [`Model`].
pub fn decode(data: &Value) -> Result<Model, DecodeError> {
    let arr = data
        .as_array()
        .ok_or_else(|| DecodeError::Format("expected array".into()))?;
    if arr.len() < 2 {
        return Err(DecodeError::Format("expected 2-element array".into()));
    }

    let clock_val = &arr[0];
    let root_val = &arr[1];

    let is_server = clock_val.is_number();
    let mut dec = DecodeState::new_empty();

    let model = if is_server {
        let server_time = clock_val
            .as_u64()
            .ok_or_else(|| DecodeError::Format("server time must be u64".into()))?;
        dec = DecodeState::Server(server_time);
        Model::new_server(server_time)
    } else {
        let flat_arr = clock_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("clock table must be array".into()))?;
        let flat: Vec<u64> = flat_arr.iter().map(|v| v.as_u64().unwrap_or(0)).collect();
        let cd = ClockDecoder::from_arr(&flat)
            .ok_or_else(|| DecodeError::Format("clock table too short".into()))?;
        let clock = cd.clock.clone();
        dec = DecodeState::Logical(cd);
        Model::new_from_clock(clock)
    };

    let mut model = model;

    // Decode root
    if root_val.as_u64() != Some(0) && !root_val.is_null() {
        let node_id = decode_node_into(&root_val, &mut model, &mut dec)?;
        model.root.val = node_id;
    }

    Ok(model)
}

enum DecodeState {
    Empty,
    Server(u64),
    Logical(ClockDecoder),
}

impl DecodeState {
    fn new_empty() -> Self {
        Self::Empty
    }

    fn decode_ts(&self, val: &Value) -> Result<Ts, DecodeError> {
        match self {
            DecodeState::Server(server_time) => {
                if let Some(offset) = val.as_u64() {
                    Ok(mk_ts(SESSION::SERVER, server_time - offset))
                } else if let Some(arr) = val.as_array() {
                    if arr.len() >= 2 {
                        let sid = arr[0].as_u64().unwrap_or(0);
                        let time = arr[1].as_u64().unwrap_or(0);
                        Ok(mk_ts(sid, time))
                    } else {
                        Err(DecodeError::Format("timestamp array too short".into()))
                    }
                } else {
                    Err(DecodeError::Format("invalid timestamp".into()))
                }
            }
            DecodeState::Logical(cd) => {
                if let Some(n) = val.as_i64() {
                    if n >= 0 {
                        // Server offset encoded in logical mode — shouldn't happen
                        // but handle gracefully
                        Ok(mk_ts(SESSION::SERVER, n as u64))
                    } else {
                        Err(DecodeError::Format(
                            "negative scalar in logical mode".into(),
                        ))
                    }
                } else if let Some(arr) = val.as_array() {
                    if arr.len() < 2 {
                        return Err(DecodeError::Format("timestamp array too short".into()));
                    }
                    let first = arr[0].as_i64().unwrap_or(0);
                    let second = arr[1].as_u64().unwrap_or(0);
                    if first < 0 {
                        // Logical timestamp: [-session_index, time_diff]
                        let session_index = (-first) as u32;
                        cd.decode_id(session_index, second)
                            .ok_or(DecodeError::InvalidSessionIndex)
                    } else {
                        // System session: [sid, time] (positive first element)
                        Ok(mk_ts(first as u64, second))
                    }
                } else {
                    Err(DecodeError::Format(
                        "invalid timestamp in logical mode".into(),
                    ))
                }
            }
            DecodeState::Empty => Err(DecodeError::NoClockDecoder),
        }
    }
}

fn decode_node_into(
    val: &Value,
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    let arr = val
        .as_array()
        .ok_or_else(|| DecodeError::Format("node must be array".into()))?;
    if arr.is_empty() {
        return Err(DecodeError::Format("empty node array".into()));
    }
    let type_code = arr[0]
        .as_u64()
        .ok_or_else(|| DecodeError::Format("node type must be integer".into()))?;

    match type_code {
        t if t == CON as u64 => decode_con(arr, model, state),
        t if t == VAL as u64 => decode_val(arr, model, state),
        t if t == OBJ as u64 => decode_obj(arr, model, state),
        t if t == VEC as u64 => decode_vec(arr, model, state),
        t if t == STR as u64 => decode_str(arr, model, state),
        t if t == BIN as u64 => decode_bin(arr, model, state),
        t if t == ARR as u64 => decode_arr(arr, model, state),
        other => Err(DecodeError::UnknownNodeType(other)),
    }
}

fn decode_con(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;

    let val = if arr.len() > 3 {
        // Special: arr[2] == 0 and arr[3] is either 0 (undefined) or a ts
        let special = &arr[3];
        if special.as_u64() == Some(0) {
            ConValue::Val(PackValue::Undefined)
        } else {
            let ref_ts = state.decode_ts(special)?;
            ConValue::Ref(ref_ts)
        }
    } else {
        // Normal: arr[2] is the value
        let pv = json_to_pack_value(&arr[2]);
        ConValue::Val(pv)
    };

    use crate::json_crdt::nodes::ConNode;
    let node = CrdtNode::Con(ConNode::new(id, val));
    model.index.insert(TsKey::from(id), node);
    Ok(id)
}

fn decode_val(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let child_id = decode_node_into(&arr[2], model, state)?;

    use crate::json_crdt::nodes::ValNode;
    let mut node = ValNode::new(id);
    node.val = child_id;
    model.index.insert(TsKey::from(id), CrdtNode::Val(node));
    Ok(id)
}

fn decode_obj(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let map = arr[2]
        .as_object()
        .ok_or_else(|| DecodeError::Format("obj map must be object".into()))?;

    use crate::json_crdt::nodes::ObjNode;
    let mut node = ObjNode::new(id);
    // Collect entries first to avoid borrow issues
    let entries: Vec<(String, Value)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    for (key, child_val) in entries {
        let child_id = decode_node_into(&child_val, model, state)?;
        node.keys.insert(key, child_id);
    }
    model.index.insert(TsKey::from(id), CrdtNode::Obj(node));
    Ok(id)
}

fn decode_vec(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let elements_arr = arr[2]
        .as_array()
        .ok_or_else(|| DecodeError::Format("vec elements must be array".into()))?
        .clone();

    use crate::json_crdt::nodes::VecNode;
    let mut node = VecNode::new(id);
    for elem in &elements_arr {
        if elem.as_u64() == Some(0) {
            node.elements.push(None);
        } else {
            let child_id = decode_node_into(elem, model, state)?;
            node.elements.push(Some(child_id));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Vec(node));
    Ok(id)
}

fn decode_str(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let chunks_arr = arr[2]
        .as_array()
        .ok_or_else(|| DecodeError::Format("str chunks must be array".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::StrNode;

    let mut node = StrNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_arr = chunk_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("str chunk must be array".into()))?;
        if chunk_arr.len() < 2 {
            return Err(DecodeError::Format("str chunk too short".into()));
        }
        let chunk_id = state.decode_ts(&chunk_arr[0])?;
        let content = &chunk_arr[1];
        if let Some(span) = content.as_u64() {
            // Tombstone
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(s) = content.as_str() {
            let span = s.chars().count() as u64;
            node.rga
                .push_chunk(Chunk::new(chunk_id, span, s.to_string()));
        } else {
            return Err(DecodeError::Format(
                "str chunk content must be string or number".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Str(node));
    Ok(id)
}

fn decode_bin(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let chunks_arr = arr[2]
        .as_array()
        .ok_or_else(|| DecodeError::Format("bin chunks must be array".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::BinNode;

    let mut node = BinNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_arr = chunk_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("bin chunk must be array".into()))?;
        if chunk_arr.len() < 2 {
            return Err(DecodeError::Format("bin chunk too short".into()));
        }
        let chunk_id = state.decode_ts(&chunk_arr[0])?;
        let content = &chunk_arr[1];
        if let Some(span) = content.as_u64() {
            // Tombstone
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(bytes_arr) = content.as_array() {
            let data: Vec<u8> = bytes_arr
                .iter()
                .map(|b| b.as_u64().unwrap_or(0) as u8)
                .collect();
            let span = data.len() as u64;
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        } else {
            return Err(DecodeError::Format(
                "bin chunk content must be array or number".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Bin(node));
    Ok(id)
}

fn decode_arr(
    arr: &[Value],
    model: &mut Model,
    state: &mut DecodeState,
) -> Result<Ts, DecodeError> {
    if arr.len() < 3 {
        return Err(DecodeError::MissingField);
    }
    let id = state.decode_ts(&arr[1])?;
    let chunks_arr = arr[2]
        .as_array()
        .ok_or_else(|| DecodeError::Format("arr chunks must be array".into()))?
        .clone();

    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::ArrNode;

    let mut node = ArrNode::new(id);
    for chunk_val in &chunks_arr {
        let chunk_arr = chunk_val
            .as_array()
            .ok_or_else(|| DecodeError::Format("arr chunk must be array".into()))?;
        if chunk_arr.len() < 2 {
            return Err(DecodeError::Format("arr chunk too short".into()));
        }
        let chunk_id = state.decode_ts(&chunk_arr[0])?;
        let content = &chunk_arr[1];
        if let Some(span) = content.as_u64() {
            // Tombstone
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else if let Some(node_vals) = content.as_array() {
            let node_vals = node_vals.clone();
            let mut ids = Vec::new();
            for child_val in &node_vals {
                let child_id = decode_node_into(child_val, model, state)?;
                ids.push(child_id);
            }
            let span = ids.len() as u64;
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        } else {
            return Err(DecodeError::Format(
                "arr chunk content must be array or number".into(),
            ));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Arr(node));
    Ok(id)
}

// ── Helper: serde_json::Value → PackValue ─────────────────────────────────

fn json_to_pack_value(v: &Value) -> PackValue {
    PackValue::from(v.clone())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;
    use serde_json::json;

    fn sid() -> u64 {
        123456
    }

    fn make_simple_model() -> Model {
        let mut model = Model::new(sid());
        let s = sid();
        // new_con with value "hello"
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Str("hello".into())),
        });
        // new_val
        model.apply_operation(&Op::NewVal { id: ts(s, 2) });
        // ins_val: root = val node
        model.apply_operation(&Op::InsVal {
            id: ts(s, 3),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 2),
        });
        // ins_val: val node = con node
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ts(s, 2),
            val: ts(s, 1),
        });
        model
    }

    #[test]
    fn encode_empty_model() {
        let model = Model::new(sid());
        let encoded = encode(&model);
        let arr = encoded.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1], json!(0));
    }

    #[test]
    fn roundtrip_con_value() {
        let model = make_simple_model();
        let original_view = model.view();

        let encoded = encode(&model);
        let decoded = decode(&encoded).expect("decode should succeed");
        let decoded_view = decoded.view();

        assert_eq!(original_view, decoded_view);
    }

    #[test]
    fn roundtrip_string_model() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: crate::json_crdt::constants::ORIGIN,
            data: "world".to_string(),
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
    fn roundtrip_obj_model() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewObj { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        model.apply_operation(&Op::InsObj {
            id: ts(s, 3),
            obj: ts(s, 1),
            data: vec![("x".to_string(), ts(s, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });

        let view = model.view();
        let encoded = encode(&model);
        let decoded = decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded.view(), view);
    }
}
