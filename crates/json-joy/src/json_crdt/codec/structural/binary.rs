//! Structural binary codec (CBOR-like format using CrdtWriter/CrdtReader).
//!
//! Mirrors:
//! - `structural/binary/Encoder.ts`
//! - `structural/binary/Decoder.ts`
//! - `structural/binary/constants.ts`
//!
//! Wire format (logical clock):
//! ```text
//! [4-byte offset to clock table] [tree of nodes] [clock table]
//! ```
//!
//! Wire format (server clock):
//! ```text
//! [0x80] [vu57 server_time] [tree of nodes]
//! ```
//!
//! Each node starts with an encoded timestamp, then a type-length byte:
//! - Bits 7-5: CRDT major type (0=con, 1=val, 2=obj, 3=vec, 4=str, 5=bin, 6=arr)
//! - Bits 4-0: inline length (0-30), or 31 = read extended vu57
//!
//! Timestamps in logical mode are encoded via `writer.id(session_index, time_diff)`.
//! Timestamps in server mode are encoded as plain `vu57(time)`.

use crate::json_crdt::constants::UNDEFINED_TS;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, ObjNode, StrNode, TsKey, ValNode, VecNode,
};
use crate::json_crdt_patch::clock::{ts as mk_ts, Ts};
use crate::json_crdt_patch::codec::clock::{ClockDecoder, ClockEncoder};
use crate::json_crdt_patch::enums::{JsonCrdtDataType, SESSION};
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::util::binary::{CrdtReader, CrdtWriter};
use json_joy_json_pack::PackValue;

// ── CRDT major type constants ───────────────────────────────────────────────

const MAJOR_CON: u8 = (JsonCrdtDataType::Con as u8) << 5;
const MAJOR_VAL: u8 = (JsonCrdtDataType::Val as u8) << 5;
const MAJOR_OBJ: u8 = (JsonCrdtDataType::Obj as u8) << 5;
const MAJOR_VEC: u8 = (JsonCrdtDataType::Vec as u8) << 5;
const MAJOR_STR: u8 = (JsonCrdtDataType::Str as u8) << 5;
const MAJOR_BIN: u8 = (JsonCrdtDataType::Bin as u8) << 5;
const MAJOR_ARR: u8 = (JsonCrdtDataType::Arr as u8) << 5;

// ── Encode ──────────────────────────────────────────────────────────────────

/// Encode a [`Model`] to the structural binary format.
pub fn encode(model: &Model) -> Vec<u8> {
    let is_server = model.clock.sid == SESSION::SERVER;
    let mut w = CrdtWriter::new();

    if is_server {
        encode_server(model, &mut w);
    } else {
        encode_logical(model, &mut w);
    }
    w.flush()
}

fn encode_server(model: &Model, w: &mut CrdtWriter) {
    let server_time = model.clock.time;
    w.u8(0x80);
    w.vu57(server_time);
    encode_root_server(model, w, server_time);
}

fn encode_logical(model: &Model, w: &mut CrdtWriter) {
    let mut enc = ClockEncoder::new();
    enc.reset(&model.clock);

    // Reserve 4 bytes for the clock-table offset
    w.ensure_capacity(4);
    let offset_pos = w.inner.x;
    w.inner.x += 4;

    // Encode the tree
    let tree_start = w.inner.x;
    encode_root_logical(model, w, &mut enc);
    let tree_end = w.inner.x;

    // Write clock table
    let table_offset = tree_end - tree_start;
    // Write the offset into the reserved 4 bytes (big-endian u32)
    let off_bytes = (table_offset as u32).to_be_bytes();
    w.inner.uint8[offset_pos..offset_pos + 4].copy_from_slice(&off_bytes);

    // Encode clock table
    let flat = enc.to_json();
    let n = flat.len() / 2; // number of entries
    w.vu57(n as u64);
    let mut i = 0;
    while i + 1 < flat.len() {
        w.vu57(flat[i]); // sid
        w.vu57(flat[i + 1]); // time
        i += 2;
    }
}

fn encode_root_server(model: &Model, w: &mut CrdtWriter, server_time: u64) {
    let root_ts = model.root.val;
    if root_ts == UNDEFINED_TS || root_ts.time == 0 {
        w.u8(0);
    } else {
        if let Some(node) = model.index.get(&TsKey::from(root_ts)) {
            encode_node_server(model, node, w, server_time);
        } else {
            w.u8(0);
        }
    }
}

fn encode_root_logical(model: &Model, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    let root_ts = model.root.val;
    if root_ts == UNDEFINED_TS || root_ts.time == 0 {
        w.u8(0);
    } else {
        if let Some(node) = model.index.get(&TsKey::from(root_ts)) {
            encode_node_logical(model, node, w, enc);
        } else {
            w.u8(0);
        }
    }
}

fn write_tl(w: &mut CrdtWriter, major: u8, length: usize) {
    if length < 31 {
        w.u8(major | length as u8);
    } else {
        w.u8(major | 31);
        w.vu57(length as u64);
    }
}

// ── Server-clock encoding helpers ─────────────────────────────────────────

fn ts_server(w: &mut CrdtWriter, stamp: Ts) {
    w.vu57(stamp.time);
}

fn encode_node_server(model: &Model, node: &CrdtNode, w: &mut CrdtWriter, server_time: u64) {
    match node {
        CrdtNode::Con(n) => encode_con_server(n, w),
        CrdtNode::Val(n) => encode_val_server(model, n, w, server_time),
        CrdtNode::Obj(n) => encode_obj_server(model, n, w, server_time),
        CrdtNode::Vec(n) => encode_vec_server(model, n, w, server_time),
        CrdtNode::Str(n) => encode_str_server(n, w),
        CrdtNode::Bin(n) => encode_bin_server(n, w),
        CrdtNode::Arr(n) => encode_arr_server(model, n, w, server_time),
    }
}

fn encode_con_server(node: &ConNode, w: &mut CrdtWriter) {
    ts_server(w, node.id);
    match &node.val {
        ConValue::Ref(ref_ts) => {
            w.u8(MAJOR_CON | 1);
            ts_server(w, *ref_ts);
        }
        ConValue::Val(pv) => {
            w.u8(MAJOR_CON | 0);
            write_cbor_value(w, pv);
        }
    }
}

fn encode_val_server(model: &Model, node: &ValNode, w: &mut CrdtWriter, server_time: u64) {
    ts_server(w, node.id);
    w.u8(MAJOR_VAL | 0);
    if let Some(child) = model.index.get(&TsKey::from(node.val)) {
        encode_node_server(model, child, w, server_time);
    }
}

fn encode_obj_server(model: &Model, node: &ObjNode, w: &mut CrdtWriter, server_time: u64) {
    ts_server(w, node.id);
    write_tl(w, MAJOR_OBJ, node.keys.len());
    for (key, &child_ts) in &node.keys {
        write_cbor_str(w, key);
        if let Some(child) = model.index.get(&TsKey::from(child_ts)) {
            encode_node_server(model, child, w, server_time);
        }
    }
}

fn encode_vec_server(model: &Model, node: &VecNode, w: &mut CrdtWriter, server_time: u64) {
    ts_server(w, node.id);
    write_tl(w, MAJOR_VEC, node.elements.len());
    for elem in &node.elements {
        match elem {
            None => w.u8(0),
            Some(id) => {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node_server(model, child, w, server_time);
                } else {
                    w.u8(0);
                }
            }
        }
    }
}

fn encode_str_server(node: &StrNode, w: &mut CrdtWriter) {
    ts_server(w, node.id);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_STR, count);
    for chunk in node.rga.iter() {
        ts_server(w, chunk.id);
        if chunk.deleted {
            write_cbor_uint(w, chunk.span);
        } else {
            write_cbor_str(w, chunk.data.as_deref().unwrap_or(""));
        }
    }
}

fn encode_bin_server(node: &BinNode, w: &mut CrdtWriter) {
    ts_server(w, node.id);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_BIN, count);
    for chunk in node.rga.iter() {
        ts_server(w, chunk.id);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            w.buf(chunk.data.as_deref().unwrap_or(&[]));
        }
    }
}

fn encode_arr_server(model: &Model, node: &ArrNode, w: &mut CrdtWriter, server_time: u64) {
    ts_server(w, node.id);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_ARR, count);
    for chunk in node.rga.iter() {
        ts_server(w, chunk.id);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            let ids = chunk.data.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
            for id in ids {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node_server(model, child, w, server_time);
                }
            }
        }
    }
}

// ── Logical-clock encoding helpers ─────────────────────────────────────────

fn ts_logical(w: &mut CrdtWriter, stamp: Ts, enc: &mut ClockEncoder) {
    match enc.append(stamp) {
        Ok(rel) => w.id(rel.session_index as u64, rel.time_diff),
        Err(_) => w.id(0, 0),
    }
}

fn encode_node_logical(model: &Model, node: &CrdtNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    match node {
        CrdtNode::Con(n) => encode_con_logical(n, w, enc),
        CrdtNode::Val(n) => encode_val_logical(model, n, w, enc),
        CrdtNode::Obj(n) => encode_obj_logical(model, n, w, enc),
        CrdtNode::Vec(n) => encode_vec_logical(model, n, w, enc),
        CrdtNode::Str(n) => encode_str_logical(n, w, enc),
        CrdtNode::Bin(n) => encode_bin_logical(n, w, enc),
        CrdtNode::Arr(n) => encode_arr_logical(model, n, w, enc),
    }
}

fn encode_con_logical(node: &ConNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    match &node.val {
        ConValue::Ref(ref_ts) => {
            w.u8(MAJOR_CON | 1);
            ts_logical(w, *ref_ts, enc);
        }
        ConValue::Val(pv) => {
            w.u8(MAJOR_CON | 0);
            write_cbor_value(w, pv);
        }
    }
}

fn encode_val_logical(model: &Model, node: &ValNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    w.u8(MAJOR_VAL | 0);
    if let Some(child) = model.index.get(&TsKey::from(node.val)) {
        encode_node_logical(model, child, w, enc);
    }
}

fn encode_obj_logical(model: &Model, node: &ObjNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    write_tl(w, MAJOR_OBJ, node.keys.len());
    for (key, &child_ts) in &node.keys {
        write_cbor_str(w, key);
        if let Some(child) = model.index.get(&TsKey::from(child_ts)) {
            encode_node_logical(model, child, w, enc);
        }
    }
}

fn encode_vec_logical(model: &Model, node: &VecNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    write_tl(w, MAJOR_VEC, node.elements.len());
    for elem in &node.elements {
        match elem {
            None => w.u8(0),
            Some(id) => {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node_logical(model, child, w, enc);
                } else {
                    w.u8(0);
                }
            }
        }
    }
}

fn encode_str_logical(node: &StrNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_STR, count);
    for chunk in node.rga.iter() {
        ts_logical(w, chunk.id, enc);
        if chunk.deleted {
            write_cbor_uint(w, chunk.span);
        } else {
            write_cbor_str(w, chunk.data.as_deref().unwrap_or(""));
        }
    }
}

fn encode_bin_logical(node: &BinNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_BIN, count);
    for chunk in node.rga.iter() {
        ts_logical(w, chunk.id, enc);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            w.buf(chunk.data.as_deref().unwrap_or(&[]));
        }
    }
}

fn encode_arr_logical(model: &Model, node: &ArrNode, w: &mut CrdtWriter, enc: &mut ClockEncoder) {
    ts_logical(w, node.id, enc);
    let count = node.rga.chunk_count();
    write_tl(w, MAJOR_ARR, count);
    for chunk in node.rga.iter() {
        ts_logical(w, chunk.id, enc);
        let deleted = chunk.deleted;
        let span = chunk.span;
        w.b1vu56(deleted as u8, span);
        if !deleted {
            let ids = chunk.data.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
            for id in ids {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node_logical(model, child, w, enc);
                }
            }
        }
    }
}

// ── CBOR primitive writers ─────────────────────────────────────────────────

fn write_cbor_value(w: &mut CrdtWriter, pv: &PackValue) {
    use json_joy_json_pack::PackValue as PV;
    match pv {
        PV::Null => w.u8(0xF6),      // CBOR null
        PV::Undefined => w.u8(0xF7), // CBOR undefined
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
            // 64-bit float
            w.u8(0xFB);
            let bytes = f.to_be_bytes();
            w.buf(&bytes);
        }
        PV::Str(s) => write_cbor_str(w, s),
        PV::Bytes(b) => {
            // CBOR byte string
            let len = b.len();
            if len <= 23 {
                w.u8(0x40 | len as u8);
            } else if len <= 0xFF {
                w.u8(0x58);
                w.u8(len as u8);
            } else {
                w.u8(0x59);
                w.buf(&(len as u16).to_be_bytes());
            }
            w.buf(b);
        }
        PV::Array(arr) => {
            let len = arr.len();
            write_cbor_arr_header(w, len);
            for item in arr {
                write_cbor_value(w, item);
            }
        }
        PV::Object(map) => {
            let len = map.len();
            write_cbor_map_header(w, len);
            for (k, v) in map {
                write_cbor_str(w, k);
                write_cbor_value(w, v);
            }
        }
        PV::UInteger(n) => write_cbor_uint(w, *n),
        PV::BigInt(n) => {
            if *n >= 0 {
                write_cbor_uint(w, *n as u64);
            } else {
                write_cbor_neg(w, (-1 - n) as u64);
            }
        }
        PV::Extension(_) | PV::Blob(_) => w.u8(0xF6), // null fallback
    }
}

fn write_cbor_uint(w: &mut CrdtWriter, n: u64) {
    if n <= 23 {
        w.u8(n as u8);
    } else if n <= 0xFF {
        w.u8(0x18);
        w.u8(n as u8);
    } else if n <= 0xFFFF {
        w.u8(0x19);
        w.buf(&(n as u16).to_be_bytes());
    } else if n <= 0xFFFF_FFFF {
        w.u8(0x1A);
        w.buf(&(n as u32).to_be_bytes());
    } else {
        w.u8(0x1B);
        w.buf(&n.to_be_bytes());
    }
}

fn write_cbor_neg(w: &mut CrdtWriter, n: u64) {
    // n = -1 - actual_value, so CBOR neg = 0x20 | ...
    if n <= 23 {
        w.u8(0x20 | n as u8);
    } else if n <= 0xFF {
        w.u8(0x38);
        w.u8(n as u8);
    } else if n <= 0xFFFF {
        w.u8(0x39);
        w.buf(&(n as u16).to_be_bytes());
    } else {
        w.u8(0x3A);
        w.buf(&(n as u32).to_be_bytes());
    }
}

fn write_cbor_str(w: &mut CrdtWriter, s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len <= 23 {
        w.u8(0x60 | len as u8);
    } else if len <= 0xFF {
        w.u8(0x78);
        w.u8(len as u8);
    } else if len <= 0xFFFF {
        w.u8(0x79);
        w.buf(&(len as u16).to_be_bytes());
    } else {
        w.u8(0x7A);
        w.buf(&(len as u32).to_be_bytes());
    }
    w.buf(bytes);
}

fn write_cbor_arr_header(w: &mut CrdtWriter, len: usize) {
    if len <= 23 {
        w.u8(0x80 | len as u8);
    } else if len <= 0xFF {
        w.u8(0x98);
        w.u8(len as u8);
    } else {
        w.u8(0x99);
        w.buf(&(len as u16).to_be_bytes());
    }
}

fn write_cbor_map_header(w: &mut CrdtWriter, len: usize) {
    if len <= 23 {
        w.u8(0xA0 | len as u8);
    } else if len <= 0xFF {
        w.u8(0xB8);
        w.u8(len as u8);
    } else {
        w.u8(0xB9);
        w.buf(&(len as u16).to_be_bytes());
    }
}

// ── Decode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during binary decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("unknown node major type: {0}")]
    UnknownMajor(u8),
    #[error("invalid UTF-8")]
    InvalidUtf8,
    #[error("clock decoder not initialised")]
    NoClockDecoder,
    #[error("invalid clock table")]
    InvalidClockTable,
    #[error("format error: {0}")]
    Format(String),
}

/// Decode a structural binary document back into a [`Model`].
pub fn decode(data: &[u8]) -> Result<Model, DecodeError> {
    if data.is_empty() {
        return Err(DecodeError::EndOfInput);
    }
    let is_server = data[0] & 0x80 != 0;
    if is_server {
        decode_server(data)
    } else {
        decode_logical(data)
    }
}

fn decode_server(data: &[u8]) -> Result<Model, DecodeError> {
    let mut r = CrdtReader::new(data);
    r.u8(); // skip 0x80
    let server_time = r.vu57();
    let mut model = Model::new_server(server_time);
    let root = decode_root_server(&mut r, &mut model, server_time)?;
    model.root.val = root;
    Ok(model)
}

fn decode_logical(data: &[u8]) -> Result<Model, DecodeError> {
    let mut r = CrdtReader::new(data);
    // Read 4-byte offset to clock table
    if data.len() < 4 {
        return Err(DecodeError::EndOfInput);
    }
    let offset_bytes = r.buf(4);
    let clock_table_offset = u32::from_be_bytes([
        offset_bytes[0],
        offset_bytes[1],
        offset_bytes[2],
        offset_bytes[3],
    ]) as usize;

    // Save tree start position
    let tree_start = r.x;

    // Jump to clock table
    r.x = tree_start + clock_table_offset;

    // Decode clock table
    let n = r.vu57() as usize;
    if n == 0 {
        return Err(DecodeError::InvalidClockTable);
    }
    let first_sid = r.vu57();
    let first_time = r.vu57();
    let mut cd = ClockDecoder::new(first_sid, first_time);
    for _ in 1..n {
        let sid = r.vu57();
        let time = r.vu57();
        cd.push_tuple(sid, time);
    }
    let clock = cd.clock.clone();
    let mut model = Model::new_from_clock(clock);

    // Return to tree position
    r.x = tree_start;

    let root = decode_root_logical(&mut r, &mut model, &cd)?;
    model.root.val = root;
    Ok(model)
}

fn decode_root_server(
    r: &mut CrdtReader,
    model: &mut Model,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    if r.x >= r.data.len() {
        return Ok(UNDEFINED_TS);
    }
    let peek = r.data[r.x];
    if peek == 0 {
        r.x += 1;
        Ok(UNDEFINED_TS)
    } else {
        decode_node_server(r, model, server_time)
    }
}

fn decode_root_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    if r.x >= r.data.len() {
        return Ok(UNDEFINED_TS);
    }
    let peek = r.data[r.x];
    if peek == 0 {
        r.x += 1;
        Ok(UNDEFINED_TS)
    } else {
        decode_node_logical(r, model, cd)
    }
}

fn read_ts_server(r: &mut CrdtReader) -> Ts {
    mk_ts(SESSION::SERVER, r.vu57())
}

fn read_ts_logical(r: &mut CrdtReader, cd: &ClockDecoder) -> Result<Ts, DecodeError> {
    let (session_index, time_diff) = r.id();
    cd.decode_id(session_index as u32, time_diff)
        .ok_or(DecodeError::Format(format!(
            "invalid session index {}",
            session_index
        )))
}

fn decode_node_server(
    r: &mut CrdtReader,
    model: &mut Model,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    let id = read_ts_server(r);
    let octet = r.u8();
    let major = octet >> 5;
    let minor = octet & 0x1F;
    let length = if minor < 31 {
        minor as usize
    } else {
        r.vu57() as usize
    };

    match major {
        0 => decode_con_server(r, model, id, length),
        1 => decode_val_server(r, model, id, server_time),
        2 => decode_obj_server(r, model, id, length, server_time),
        3 => decode_vec_server(r, model, id, length, server_time),
        4 => decode_str_server(r, model, id, length),
        5 => decode_bin_server(r, model, id, length),
        6 => decode_arr_server(r, model, id, length, server_time),
        other => Err(DecodeError::UnknownMajor(other)),
    }
}

fn decode_con_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
) -> Result<Ts, DecodeError> {
    let con_val = if length == 0 {
        let pv = read_cbor_value(r)?;
        ConValue::Val(pv)
    } else {
        let ref_ts = read_ts_server(r);
        ConValue::Ref(ref_ts)
    };
    use crate::json_crdt::nodes::ConNode;
    model
        .index
        .insert(TsKey::from(id), CrdtNode::Con(ConNode::new(id, con_val)));
    Ok(id)
}

fn decode_val_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    let child_id = decode_node_server(r, model, server_time)?;
    use crate::json_crdt::nodes::ValNode;
    let mut node = ValNode::new(id);
    node.val = child_id;
    model.index.insert(TsKey::from(id), CrdtNode::Val(node));
    Ok(id)
}

fn decode_obj_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::ObjNode;
    let mut node = ObjNode::new(id);
    for _ in 0..length {
        let key = read_cbor_str(r)?;
        let child_id = decode_node_server(r, model, server_time)?;
        node.keys.insert(key, child_id);
    }
    model.index.insert(TsKey::from(id), CrdtNode::Obj(node));
    Ok(id)
}

fn decode_vec_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::VecNode;
    let mut node = VecNode::new(id);
    for _ in 0..length {
        let peek = r.data[r.x];
        if peek == 0 {
            r.x += 1;
            node.elements.push(None);
        } else {
            let child_id = decode_node_server(r, model, server_time)?;
            node.elements.push(Some(child_id));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Vec(node));
    Ok(id)
}

fn decode_str_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::StrNode;
    let mut node = StrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_server(r);
        let val = read_cbor_value(r)?;
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
    model.index.insert(TsKey::from(id), CrdtNode::Str(node));
    Ok(id)
}

fn decode_bin_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::BinNode;
    let mut node = BinNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_server(r);
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let data = r.buf(span as usize).to_vec();
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Bin(node));
    Ok(id)
}

fn decode_arr_server(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    server_time: u64,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::ArrNode;
    let mut node = ArrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_server(r);
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let mut ids = Vec::new();
            for _ in 0..span {
                let child_id = decode_node_server(r, model, server_time)?;
                ids.push(child_id);
            }
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Arr(node));
    Ok(id)
}

// ── Logical clock decode helpers ───────────────────────────────────────────

fn decode_node_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    let id = read_ts_logical(r, cd)?;
    let octet = r.u8();
    let major = octet >> 5;
    let minor = octet & 0x1F;
    let length = if minor < 31 {
        minor as usize
    } else {
        r.vu57() as usize
    };

    match major {
        0 => decode_con_logical(r, model, id, length, cd),
        1 => decode_val_logical(r, model, id, cd),
        2 => decode_obj_logical(r, model, id, length, cd),
        3 => decode_vec_logical(r, model, id, length, cd),
        4 => decode_str_logical(r, model, id, length, cd),
        5 => decode_bin_logical(r, model, id, length, cd),
        6 => decode_arr_logical(r, model, id, length, cd),
        other => Err(DecodeError::UnknownMajor(other)),
    }
}

fn decode_con_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    let con_val = if length == 0 {
        let pv = read_cbor_value(r)?;
        ConValue::Val(pv)
    } else {
        let ref_ts = read_ts_logical(r, cd)?;
        ConValue::Ref(ref_ts)
    };
    use crate::json_crdt::nodes::ConNode;
    model
        .index
        .insert(TsKey::from(id), CrdtNode::Con(ConNode::new(id, con_val)));
    Ok(id)
}

fn decode_val_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    let child_id = decode_node_logical(r, model, cd)?;
    use crate::json_crdt::nodes::ValNode;
    let mut node = ValNode::new(id);
    node.val = child_id;
    model.index.insert(TsKey::from(id), CrdtNode::Val(node));
    Ok(id)
}

fn decode_obj_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::ObjNode;
    let mut node = ObjNode::new(id);
    for _ in 0..length {
        let key = read_cbor_str(r)?;
        let child_id = decode_node_logical(r, model, cd)?;
        node.keys.insert(key, child_id);
    }
    model.index.insert(TsKey::from(id), CrdtNode::Obj(node));
    Ok(id)
}

fn decode_vec_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::VecNode;
    let mut node = VecNode::new(id);
    for _ in 0..length {
        let peek = r.data[r.x];
        if peek == 0 {
            r.x += 1;
            node.elements.push(None);
        } else {
            let child_id = decode_node_logical(r, model, cd)?;
            node.elements.push(Some(child_id));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Vec(node));
    Ok(id)
}

fn decode_str_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::StrNode;
    let mut node = StrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_logical(r, cd)?;
        let val = read_cbor_value(r)?;
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
    model.index.insert(TsKey::from(id), CrdtNode::Str(node));
    Ok(id)
}

fn decode_bin_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::BinNode;
    let mut node = BinNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_logical(r, cd)?;
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let data = r.buf(span as usize).to_vec();
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Bin(node));
    Ok(id)
}

fn decode_arr_logical(
    r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::ArrNode;
    let mut node = ArrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_logical(r, cd)?;
        let (deleted, span) = r.b1vu56();
        if deleted != 0 {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let mut ids = Vec::new();
            for _ in 0..span {
                let child_id = decode_node_logical(r, model, cd)?;
                ids.push(child_id);
            }
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Arr(node));
    Ok(id)
}

// ── Minimal CBOR reader ───────────────────────────────────────────────────

fn read_cbor_value(r: &mut CrdtReader) -> Result<PackValue, DecodeError> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;

    match major {
        0 => {
            // Unsigned int
            let n = read_cbor_argument(r, info)?;
            Ok(PackValue::Integer(n as i64))
        }
        1 => {
            // Negative int: -1 - n
            let n = read_cbor_argument(r, info)?;
            Ok(PackValue::Integer(-1 - n as i64))
        }
        2 => {
            // Byte string
            let len = read_cbor_argument(r, info)? as usize;
            Ok(PackValue::Bytes(r.buf(len).to_vec()))
        }
        3 => {
            // Text string
            let len = read_cbor_argument(r, info)? as usize;
            let s = r.utf8(len);
            Ok(PackValue::Str(s.to_string()))
        }
        4 => {
            // Array
            let len = read_cbor_argument(r, info)? as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(read_cbor_value(r)?);
            }
            Ok(PackValue::Array(items))
        }
        5 => {
            // Map
            let len = read_cbor_argument(r, info)? as usize;
            let mut map = Vec::with_capacity(len);
            for _ in 0..len {
                let key = read_cbor_value(r)?;
                let key_str = match key {
                    PackValue::Str(s) => s,
                    _ => String::new(),
                };
                let val = read_cbor_value(r)?;
                map.push((key_str, val));
            }
            Ok(PackValue::Object(map))
        }
        7 => {
            // Float/special
            match info {
                20 => Ok(PackValue::Bool(false)),
                21 => Ok(PackValue::Bool(true)),
                22 => Ok(PackValue::Null),
                23 => Ok(PackValue::Undefined),
                25 => {
                    let bytes = r.buf(2);
                    let bits = u16::from_be_bytes([bytes[0], bytes[1]]);
                    Ok(PackValue::Float(json_joy_buffers::decode_f16(bits) as f64))
                }
                26 => {
                    let bytes = r.buf(4);
                    let f = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    Ok(PackValue::Float(f as f64))
                }
                27 => {
                    let bytes = r.buf(8);
                    let f = f64::from_be_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ]);
                    Ok(PackValue::Float(f))
                }
                _ => Ok(PackValue::Null),
            }
        }
        _ => Err(DecodeError::Format(format!("unknown CBOR major {}", major))),
    }
}

fn read_cbor_argument(r: &mut CrdtReader, info: u8) -> Result<u64, DecodeError> {
    match info {
        n if n <= 23 => Ok(n as u64),
        24 => Ok(r.u8() as u64),
        25 => {
            let b = r.buf(2);
            Ok(u16::from_be_bytes([b[0], b[1]]) as u64)
        }
        26 => {
            let b = r.buf(4);
            Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64)
        }
        27 => {
            let b = r.buf(8);
            Ok(u64::from_be_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ]))
        }
        _ => Err(DecodeError::Format(format!(
            "unsupported CBOR additional info {}",
            info
        ))),
    }
}

fn read_cbor_str(r: &mut CrdtReader) -> Result<String, DecodeError> {
    let val = read_cbor_value(r)?;
    match val {
        PackValue::Str(s) => Ok(s),
        _ => Err(DecodeError::Format("expected string".into())),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 {
        789012
    }

    #[test]
    fn roundtrip_empty() {
        let model = Model::new(sid());
        let bytes = encode(&model);
        let decoded = decode(&bytes).expect("decode");
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
            data: "binary test".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let bytes = encode(&model);
        let decoded = decode(&bytes).expect("decode");
        assert_eq!(decoded.view(), view);
    }

    #[test]
    fn roundtrip_con_number() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view = model.view();
        let bytes = encode(&model);
        let decoded = decode(&bytes).expect("decode");
        assert_eq!(decoded.view(), view);
    }
}
