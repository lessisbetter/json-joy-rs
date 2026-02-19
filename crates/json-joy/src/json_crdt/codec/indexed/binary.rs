//! Indexed binary codec.
//!
//! Mirrors:
//! - `indexed/binary/Encoder.ts`
//! - `indexed/binary/Decoder.ts`
//! - `indexed/binary/types.ts`
//!
//! Wire format: a `HashMap<String, Vec<u8>>` where:
//! - `"c"` → clock table bytes (ClockTable written as [sid, time] pairs)
//! - `"r"` → (optional) root value timestamp bytes
//! - `"<sidIdx>_<time>"` in base-36 → encoded node bytes
//!
//! Each node is encoded using the same CBOR-like binary encoding as the
//! structural binary codec, but without the node ID prefix (since the ID
//! is encoded in the field name).
//!
//! The node encoding byte starts directly with the type-length byte:
//! - Bits 7-5: CRDT major type
//! - Bits 4-0: inline length
//!
//! References inside nodes (child IDs, etc.) are encoded as CRDT id tuples
//! `(session_index, time)` using the ClockTable for lookup.

use std::collections::HashMap;

use crate::json_crdt::constants::UNDEFINED_TS;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, IndexExt, NodeIndex, ObjNode, StrNode, TsKey,
    ValNode, VecNode,
};
use crate::json_crdt_patch::clock::{ts as mk_ts, ClockVector, Ts};
use crate::json_crdt_patch::codec::clock::ClockTable;
use crate::json_crdt_patch::enums::{JsonCrdtDataType, SESSION};
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::util::binary::{CrdtReader, CrdtWriter};
use json_joy_json_pack::PackValue;

// ── Constants ──────────────────────────────────────────────────────────────

const MAJOR_CON: u8 = (JsonCrdtDataType::Con as u8) << 5;
const MAJOR_VAL: u8 = (JsonCrdtDataType::Val as u8) << 5;
const MAJOR_OBJ: u8 = (JsonCrdtDataType::Obj as u8) << 5;
const MAJOR_VEC: u8 = (JsonCrdtDataType::Vec as u8) << 5;
const MAJOR_STR: u8 = (JsonCrdtDataType::Str as u8) << 5;
const MAJOR_BIN: u8 = (JsonCrdtDataType::Bin as u8) << 5;
const MAJOR_ARR: u8 = (JsonCrdtDataType::Arr as u8) << 5;

/// Map from field name to field bytes.
pub type IndexedFields = HashMap<String, Vec<u8>>;

// ── Encode ──────────────────────────────────────────────────────────────────

/// Encode a [`Model`] to the indexed binary format.
pub fn encode(model: &Model) -> IndexedFields {
    let table = ClockTable::from_clock(&model.clock);
    let mut fields = IndexedFields::new();

    // Encode clock table
    fields.insert("c".to_string(), encode_clock_table(&table));

    // Encode root reference
    let root_ts = model.root.val;
    if root_ts != UNDEFINED_TS && root_ts.time != 0 {
        let mut w = CrdtWriter::new();
        write_ts_indexed(&mut w, root_ts, &table);
        fields.insert("r".to_string(), w.flush());
    }

    // Encode each node
    for (key, node) in &model.index {
        let id = mk_ts(key.sid, key.time);
        let (sid_idx, _) = match table.get_by_sid(id.sid) {
            Some(entry) => entry,
            None => continue,
        };
        let time = id.time;
        // Field name: base-36 of sid index + "_" + base-36 of time
        let field_name = format!("{}_{}", to_base36(sid_idx as u64), to_base36(time));
        let bytes = encode_node(node, &table, model);
        fields.insert(field_name, bytes);
    }

    fields
}

fn encode_clock_table(table: &ClockTable) -> Vec<u8> {
    let mut w = CrdtWriter::new();
    w.vu57(table.by_idx.len() as u64);
    for entry in &table.by_idx {
        w.vu57(entry.sid);
        w.vu57(entry.time);
    }
    w.flush()
}

fn write_ts_indexed(w: &mut CrdtWriter, stamp: Ts, table: &ClockTable) {
    let (idx, ref_ts) = match table.get_by_sid(stamp.sid) {
        Some(entry) => entry,
        None => {
            w.id(0, stamp.time);
            return;
        }
    };
    // time_diff = ref_ts.time - stamp.time (if ref_ts.time >= stamp.time)
    let time_diff = if ref_ts.time >= stamp.time {
        ref_ts.time - stamp.time
    } else {
        0 // shouldn't happen in well-formed data
    };
    w.id(idx as u64, time_diff);
}

fn encode_node(node: &CrdtNode, table: &ClockTable, model: &Model) -> Vec<u8> {
    let mut w = CrdtWriter::new();
    match node {
        CrdtNode::Con(n) => encode_con(&mut w, n, table),
        CrdtNode::Val(n) => encode_val(&mut w, n, table),
        CrdtNode::Obj(n) => encode_obj(&mut w, n, table),
        CrdtNode::Vec(n) => encode_vec(&mut w, n, table),
        CrdtNode::Str(n) => encode_str(&mut w, n, table),
        CrdtNode::Bin(n) => encode_bin(&mut w, n, table),
        CrdtNode::Arr(n) => encode_arr(&mut w, n, table),
    }
    w.flush()
}

fn write_tl(w: &mut CrdtWriter, major: u8, length: usize) {
    if length < 24 {
        w.u8(major | length as u8);
    } else if length <= 0xFF {
        w.u8(major | 24);
        w.u8(length as u8);
    } else if length <= 0xFFFF {
        w.u8(major | 25);
        w.buf(&(length as u16).to_be_bytes());
    } else {
        w.u8(major | 26);
        w.buf(&(length as u32).to_be_bytes());
    }
}

fn encode_con(w: &mut CrdtWriter, node: &ConNode, table: &ClockTable) {
    match &node.val {
        ConValue::Ref(ref_ts) => {
            write_tl(w, MAJOR_CON, 1);
            write_ts_indexed(w, *ref_ts, table);
        }
        ConValue::Val(pv) => {
            write_tl(w, MAJOR_CON, 0);
            write_cbor_value(w, pv);
        }
    }
}

fn encode_val(w: &mut CrdtWriter, node: &ValNode, table: &ClockTable) {
    write_tl(w, MAJOR_VAL, 0);
    write_ts_indexed(w, node.val, table);
}

fn encode_obj(w: &mut CrdtWriter, node: &ObjNode, table: &ClockTable) {
    write_tl(w, MAJOR_OBJ, node.keys.len());
    let mut sorted_keys: Vec<&String> = node.keys.keys().collect();
    sorted_keys.sort();
    for key in &sorted_keys {
        let child_ts = node.keys[key.as_str()];
        write_cbor_str(w, key);
        write_ts_indexed(w, child_ts, table);
    }
}

fn encode_vec(w: &mut CrdtWriter, node: &VecNode, table: &ClockTable) {
    write_tl(w, MAJOR_VEC, node.elements.len());
    for elem in &node.elements {
        match elem {
            None => w.u8(0),
            Some(id) => {
                w.u8(1);
                write_ts_indexed(w, *id, table);
            }
        }
    }
}

fn encode_str(w: &mut CrdtWriter, node: &StrNode, table: &ClockTable) {
    write_tl(w, MAJOR_STR, node.rga.chunk_count());
    for chunk in node.rga.iter() {
        write_ts_indexed(w, chunk.id, table);
        if chunk.deleted {
            write_cbor_uint(w, chunk.span);
        } else {
            write_cbor_str(w, chunk.data.as_deref().unwrap_or(""));
        }
    }
}

fn encode_bin(w: &mut CrdtWriter, node: &BinNode, table: &ClockTable) {
    write_tl(w, MAJOR_BIN, node.rga.chunk_count());
    for chunk in node.rga.iter() {
        write_ts_indexed(w, chunk.id, table);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            w.buf(chunk.data.as_deref().unwrap_or(&[]));
        }
    }
}

fn encode_arr(w: &mut CrdtWriter, node: &ArrNode, table: &ClockTable) {
    write_tl(w, MAJOR_ARR, node.rga.chunk_count());
    for chunk in node.rga.iter() {
        write_ts_indexed(w, chunk.id, table);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            let ids = chunk.data.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
            for id in ids {
                write_ts_indexed(w, *id, table);
            }
        }
    }
}

// ── CBOR primitive writers ────────────────────────────────────────────────

fn write_cbor_value(w: &mut CrdtWriter, pv: &PackValue) {
    use json_joy_json_pack::PackValue as PV;
    match pv {
        PV::Null => w.u8(0xF6),
        PV::Undefined => w.u8(0xF7),
        PV::Bool(true) => w.u8(0xF5),
        PV::Bool(false) => w.u8(0xF4),
        PV::Integer(n) => {
            if *n >= 0 {
                write_cbor_uint(w, *n as u64);
            } else {
                write_cbor_neg(w, (-1 - n) as u64);
            }
        }
        PV::Float(f) => {
            w.u8(0xFB);
            w.buf(&f.to_be_bytes());
        }
        PV::Str(s) => write_cbor_str(w, s),
        PV::Bytes(b) => {
            let len = b.len();
            if len <= 23 { w.u8(0x40 | len as u8); }
            else if len <= 0xFF { w.u8(0x58); w.u8(len as u8); }
            else { w.u8(0x59); w.buf(&(len as u16).to_be_bytes()); }
            w.buf(b);
        }
        PV::Array(arr) => {
            let len = arr.len();
            if len <= 23 { w.u8(0x80 | len as u8); }
            else { w.u8(0x98); w.u8(len as u8); }
            for item in arr { write_cbor_value(w, item); }
        }
        PV::Object(map) => {
            let len = map.len();
            if len <= 23 { w.u8(0xA0 | len as u8); }
            else { w.u8(0xB8); w.u8(len as u8); }
            for (k, v) in map { write_cbor_str(w, k); write_cbor_value(w, v); }
        }
        PV::UInteger(n) => write_cbor_uint(w, *n),
        PV::BigInt(n) => {
            if *n >= 0 { write_cbor_uint(w, *n as u64); }
            else { write_cbor_neg(w, (-1 - n) as u64); }
        }
        PV::Extension(_) | PV::Blob(_) => w.u8(0xF6),
    }
}

fn write_cbor_uint(w: &mut CrdtWriter, n: u64) {
    if n <= 23 { w.u8(n as u8); }
    else if n <= 0xFF { w.u8(0x18); w.u8(n as u8); }
    else if n <= 0xFFFF { w.u8(0x19); w.buf(&(n as u16).to_be_bytes()); }
    else if n <= 0xFFFF_FFFF { w.u8(0x1A); w.buf(&(n as u32).to_be_bytes()); }
    else { w.u8(0x1B); w.buf(&n.to_be_bytes()); }
}

fn write_cbor_neg(w: &mut CrdtWriter, n: u64) {
    if n <= 23 { w.u8(0x20 | n as u8); }
    else if n <= 0xFF { w.u8(0x38); w.u8(n as u8); }
    else { w.u8(0x39); w.buf(&(n as u16).to_be_bytes()); }
}

fn write_cbor_str(w: &mut CrdtWriter, s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len <= 23 { w.u8(0x60 | len as u8); }
    else if len <= 0xFF { w.u8(0x78); w.u8(len as u8); }
    else { w.u8(0x79); w.buf(&(len as u16).to_be_bytes()); }
    w.buf(bytes);
}

// ── Decode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during indexed binary decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("missing clock field")]
    MissingClock,
    #[error("invalid clock table")]
    InvalidClockTable,
    #[error("format error: {0}")]
    Format(String),
}

/// Decode indexed binary fields back into a [`Model`].
pub fn decode(fields: &IndexedFields) -> Result<Model, DecodeError> {
    let clock_bytes = fields.get("c")
        .ok_or(DecodeError::MissingClock)?;

    let table = decode_clock_table(clock_bytes)?;

    // Build the initial vector clock from the table
    let first = table.by_idx.first()
        .ok_or(DecodeError::InvalidClockTable)?;
    let mut clock = ClockVector::new(first.sid, first.time + 1);
    for entry in &table.by_idx[1..] {
        clock.observe(*entry, 1);
    }
    let mut model = Model {
        root: crate::json_crdt::nodes::RootNode::new(),
        index: NodeIndex::default(),
        clock,
        tick: 0,
    };

    // Decode root reference
    if let Some(root_bytes) = fields.get("r") {
        if !root_bytes.is_empty() {
            let mut r = CrdtReader::new(root_bytes);
            let root_ts = read_ts_indexed(&mut r, &table)?;
            model.root.val = root_ts;
        }
    }

    // Decode all nodes
    for (field, bytes) in fields {
        if field == "c" || field == "r" {
            continue;
        }
        // Parse field name: "<sidIdx>_<time>" in base-36
        let id = parse_field_name(field, &table)?;
        let mut r = CrdtReader::new(bytes);
        let node = decode_node(&mut r, id, &table, &model.clock)?;
        model.index.insert(TsKey::from(id), node);
    }

    Ok(model)
}

fn decode_clock_table(data: &[u8]) -> Result<ClockTable, DecodeError> {
    let mut r = CrdtReader::new(data);
    let n = r.vu57() as usize;
    if n == 0 {
        return Err(DecodeError::InvalidClockTable);
    }
    let mut table = ClockTable::new();
    for _ in 0..n {
        let sid = r.vu57();
        let time = r.vu57();
        table.push(mk_ts(sid, time));
    }
    Ok(table)
}

fn read_ts_indexed(r: &mut CrdtReader, table: &ClockTable) -> Result<Ts, DecodeError> {
    let (idx, time_diff) = r.id();
    let ref_ts = table.get_by_index(idx as usize)
        .ok_or_else(|| DecodeError::Format(format!("invalid session index {}", idx)))?;
    if ref_ts.time < time_diff {
        return Err(DecodeError::Format("time underflow".into()));
    }
    Ok(mk_ts(ref_ts.sid, ref_ts.time - time_diff))
}

fn parse_field_name(name: &str, table: &ClockTable) -> Result<Ts, DecodeError> {
    let parts: Vec<&str> = name.splitn(2, '_').collect();
    if parts.len() != 2 {
        return Err(DecodeError::Format(format!("invalid field name: {}", name)));
    }
    let sid_idx = from_base36(parts[0])
        .ok_or_else(|| DecodeError::Format(format!("invalid base-36: {}", parts[0])))?;
    let time = from_base36(parts[1])
        .ok_or_else(|| DecodeError::Format(format!("invalid base-36: {}", parts[1])))?;
    let ref_ts = table.get_by_index(sid_idx as usize)
        .ok_or_else(|| DecodeError::Format(format!("invalid session index {}", sid_idx)))?;
    Ok(mk_ts(ref_ts.sid, time))
}

fn decode_node(r: &mut CrdtReader, id: Ts, table: &ClockTable, _clock: &ClockVector) -> Result<CrdtNode, DecodeError> {
    let octet = r.u8();
    let major = octet >> 5;
    let info = octet & 0x1F;
    let length = if info < 24 {
        info as usize
    } else if info == 24 {
        r.u8() as usize
    } else if info == 25 {
        let b = r.buf(2);
        u16::from_be_bytes([b[0], b[1]]) as usize
    } else {
        let b = r.buf(4);
        u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
    };

    match major {
        0 => decode_con(r, id, length, table),
        1 => decode_val(r, id, table),
        2 => decode_obj(r, id, length, table),
        3 => decode_vec(r, id, length, table),
        4 => decode_str(r, id, length, table),
        5 => decode_bin(r, id, length, table),
        6 => decode_arr(r, id, length, table),
        other => Err(DecodeError::Format(format!("unknown major type {}", other))),
    }
}

fn decode_con(r: &mut CrdtReader, id: Ts, length: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    let val = if length == 0 {
        let pv = read_cbor_value(r)
            .map_err(|e| DecodeError::Format(e.to_string()))?;
        ConValue::Val(pv)
    } else {
        let ref_ts = read_ts_indexed(r, table)?;
        ConValue::Ref(ref_ts)
    };
    Ok(CrdtNode::Con(ConNode::new(id, val)))
}

fn decode_val(r: &mut CrdtReader, id: Ts, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    let val_ts = read_ts_indexed(r, table)?;
    let mut node = ValNode::new(id);
    node.val = val_ts;
    Ok(CrdtNode::Val(node))
}

fn decode_obj(r: &mut CrdtReader, id: Ts, length: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    let mut node = ObjNode::new(id);
    for _ in 0..length {
        let key = read_cbor_str_indexed(r)
            .map_err(|e| DecodeError::Format(e.to_string()))?;
        let child_ts = read_ts_indexed(r, table)?;
        node.keys.insert(key, child_ts);
    }
    Ok(CrdtNode::Obj(node))
}

fn decode_vec(r: &mut CrdtReader, id: Ts, length: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    let mut node = VecNode::new(id);
    for _ in 0..length {
        let octet = r.u8();
        if octet == 0 {
            node.elements.push(None);
        } else {
            let child_ts = read_ts_indexed(r, table)?;
            node.elements.push(Some(child_ts));
        }
    }
    Ok(CrdtNode::Vec(node))
}

fn decode_str(r: &mut CrdtReader, id: Ts, count: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    use crate::json_crdt::nodes::StrNode;
    use crate::json_crdt::nodes::rga::Chunk;
    let mut node = StrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_indexed(r, table)?;
        let val = read_cbor_value(r)
            .map_err(|e| DecodeError::Format(e.to_string()))?;
        match val {
            PackValue::Integer(n) if n >= 0 => {
                node.rga.push_chunk(Chunk::new_deleted(chunk_id, n as u64));
            }
            PackValue::Str(s) => {
                let span = s.chars().count() as u64;
                node.rga.push_chunk(Chunk::new(chunk_id, span, s));
            }
            _ => {}
        }
    }
    Ok(CrdtNode::Str(node))
}

fn decode_bin(r: &mut CrdtReader, id: Ts, count: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    use crate::json_crdt::nodes::BinNode;
    use crate::json_crdt::nodes::rga::Chunk;
    let mut node = BinNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_indexed(r, table)?;
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let data = r.buf(span as usize).to_vec();
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        }
    }
    Ok(CrdtNode::Bin(node))
}

fn decode_arr(r: &mut CrdtReader, id: Ts, count: usize, table: &ClockTable) -> Result<CrdtNode, DecodeError> {
    use crate::json_crdt::nodes::ArrNode;
    use crate::json_crdt::nodes::rga::Chunk;
    let mut node = ArrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_indexed(r, table)?;
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let mut ids = Vec::new();
            for _ in 0..span {
                let child_ts = read_ts_indexed(r, table)?;
                ids.push(child_ts);
            }
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        }
    }
    Ok(CrdtNode::Arr(node))
}

// ── CBOR reader helpers ───────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum CborError {
    #[error("format: {0}")]
    Format(String),
}

fn read_cbor_value(r: &mut CrdtReader) -> Result<PackValue, CborError> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;

    match major {
        0 => {
            let n = read_cbor_argument(r, info)?;
            Ok(PackValue::Integer(n as i64))
        }
        1 => {
            let n = read_cbor_argument(r, info)?;
            Ok(PackValue::Integer(-1 - n as i64))
        }
        2 => {
            let len = read_cbor_argument(r, info)? as usize;
            Ok(PackValue::Bytes(r.buf(len).to_vec()))
        }
        3 => {
            let len = read_cbor_argument(r, info)? as usize;
            Ok(PackValue::Str(r.utf8(len).to_string()))
        }
        4 => {
            let len = read_cbor_argument(r, info)? as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len { items.push(read_cbor_value(r)?); }
            Ok(PackValue::Array(items))
        }
        5 => {
            let len = read_cbor_argument(r, info)? as usize;
            let mut map = Vec::with_capacity(len);
            for _ in 0..len {
                let k = match read_cbor_value(r)? { PackValue::Str(s) => s, _ => String::new() };
                let v = read_cbor_value(r)?;
                map.push((k, v));
            }
            Ok(PackValue::Object(map))
        }
        7 => match info {
            20 => Ok(PackValue::Bool(false)),
            21 => Ok(PackValue::Bool(true)),
            22 => Ok(PackValue::Null),
            23 => Ok(PackValue::Undefined),
            27 => {
                let b = r.buf(8);
                Ok(PackValue::Float(f64::from_be_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])))
            }
            _ => Ok(PackValue::Null),
        },
        _ => Err(CborError::Format(format!("unknown major {}", major))),
    }
}

fn read_cbor_argument(r: &mut CrdtReader, info: u8) -> Result<u64, CborError> {
    match info {
        n if n <= 23 => Ok(n as u64),
        24 => Ok(r.u8() as u64),
        25 => { let b = r.buf(2); Ok(u16::from_be_bytes([b[0], b[1]]) as u64) }
        26 => { let b = r.buf(4); Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64) }
        27 => { let b = r.buf(8); Ok(u64::from_be_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])) }
        _ => Err(CborError::Format(format!("unsupported info {}", info))),
    }
}

fn read_cbor_str_indexed(r: &mut CrdtReader) -> Result<String, CborError> {
    match read_cbor_value(r)? {
        PackValue::Str(s) => Ok(s),
        _ => Err(CborError::Format("expected string".into())),
    }
}

// ── Base-36 helpers ────────────────────────────────────────────────────────

fn to_base36(n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut result = Vec::new();
    let mut n = n;
    while n > 0 {
        result.push(CHARS[(n % 36) as usize]);
        n /= 36;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}

fn from_base36(s: &str) -> Option<u64> {
    let mut n: u64 = 0;
    for c in s.chars() {
        let digit = c.to_digit(36)?;
        n = n.checked_mul(36)?.checked_add(digit as u64)?;
    }
    Some(n)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 { 333444 }

    #[test]
    fn base36_roundtrip() {
        for n in [0u64, 1, 35, 36, 1000, 999999] {
            let s = to_base36(n);
            assert_eq!(from_base36(&s), Some(n), "n={}", n);
        }
    }

    #[test]
    fn roundtrip_empty() {
        let model = Model::new(sid());
        let fields = encode(&model);
        assert!(fields.contains_key("c"));
        let decoded = decode(&fields).expect("decode");
        assert_eq!(decoded.view(), model.view());
    }

    #[test]
    fn roundtrip_string() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: crate::json_crdt::constants::ORIGIN,
            data: "indexed".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let fields = encode(&model);
        let decoded = decode(&fields).expect("decode");
        assert_eq!(decoded.view(), view);
    }

    #[test]
    fn roundtrip_con_number() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(77)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let fields = encode(&model);
        let decoded = decode(&fields).expect("decode");
        assert_eq!(decoded.view(), view);
    }
}
