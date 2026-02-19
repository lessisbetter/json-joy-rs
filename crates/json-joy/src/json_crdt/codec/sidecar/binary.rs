//! Sidecar binary codec.
//!
//! Mirrors:
//! - `sidecar/binary/Encoder.ts`
//! - `sidecar/binary/Decoder.ts`
//!
//! This codec splits the CRDT document into two byte arrays:
//! - `view`: the plain JSON values (CBOR-encoded leaves)
//! - `meta`: the CRDT metadata (timestamps, structure) in the same binary
//!   format as the structural binary codec, but using a logical clock.
//!
//! The `meta` format is identical to the logical structural binary format
//! (4-byte clock table offset + tree of metadata + clock table), except that
//! for nodes that have a "view" value (Con, Str, Bin), the view bytes are
//! written to the `view` stream instead of the `meta` stream.
//!
//! The decoder reconstructs the document by replaying the meta stream and
//! reading view values from the view stream at the appropriate positions.

use crate::json_crdt::constants::UNDEFINED_TS;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, IndexExt, NodeIndex, ObjNode, StrNode, TsKey, ValNode,
    VecNode,
};
use crate::json_crdt_patch::clock::{ts as mk_ts, ClockVector, Ts};
use crate::json_crdt_patch::codec::clock::{ClockDecoder, ClockEncoder};
use crate::json_crdt_patch::enums::{JsonCrdtDataType, SESSION};
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::util::binary::{CrdtReader, CrdtWriter};
use json_joy_json_pack::PackValue;

const MAJOR_CON: u8 = (JsonCrdtDataType::Con as u8) << 5;
const MAJOR_VAL: u8 = (JsonCrdtDataType::Val as u8) << 5;
const MAJOR_OBJ: u8 = (JsonCrdtDataType::Obj as u8) << 5;
const MAJOR_VEC: u8 = (JsonCrdtDataType::Vec as u8) << 5;
const MAJOR_STR: u8 = (JsonCrdtDataType::Str as u8) << 5;
const MAJOR_BIN: u8 = (JsonCrdtDataType::Bin as u8) << 5;
const MAJOR_ARR: u8 = (JsonCrdtDataType::Arr as u8) << 5;

// ── Encode ──────────────────────────────────────────────────────────────────

/// Encode a [`Model`] into two byte arrays: `(view, meta)`.
pub fn encode(model: &Model) -> (Vec<u8>, Vec<u8>) {
    let mut view_w = CrdtWriter::new();
    let mut meta_w = CrdtWriter::new();
    let mut enc = ClockEncoder::new();
    enc.reset(&model.clock);

    // Reserve 4 bytes in meta for the clock-table offset
    meta_w.ensure_capacity(4);
    let offset_pos = meta_w.inner.x;
    meta_w.inner.x += 4;

    let tree_start = meta_w.inner.x;
    encode_root(model, &mut view_w, &mut meta_w, &mut enc);
    let tree_end = meta_w.inner.x;

    // Write clock table offset
    let table_offset = (tree_end - tree_start) as u32;
    let off_bytes = table_offset.to_be_bytes();
    meta_w.inner.uint8[offset_pos..offset_pos + 4].copy_from_slice(&off_bytes);

    // Write clock table
    let flat = enc.to_json();
    let n = flat.len() / 2;
    meta_w.vu57(n as u64);
    let mut i = 0;
    while i + 1 < flat.len() {
        meta_w.vu57(flat[i]);
        meta_w.vu57(flat[i + 1]);
        i += 2;
    }

    (view_w.flush(), meta_w.flush())
}

fn encode_root(
    model: &Model,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    let root_ts = model.root.val;
    if root_ts == UNDEFINED_TS || root_ts.time == 0 {
        meta_w.u8(0);
    } else {
        if let Some(node) = model.index.get(&TsKey::from(root_ts)) {
            encode_node(model, node, view_w, meta_w, enc);
        } else {
            meta_w.u8(0);
        }
    }
}

fn ts_logical(meta_w: &mut CrdtWriter, stamp: Ts, enc: &mut ClockEncoder) {
    match enc.append(stamp) {
        Ok(rel) => meta_w.id(rel.session_index as u64, rel.time_diff),
        Err(_) => meta_w.id(0, 0),
    }
}

fn write_tl(meta_w: &mut CrdtWriter, major: u8, length: usize) {
    if length < 24 {
        meta_w.u8(major | length as u8);
    } else if length <= 0xFF {
        meta_w.u8(major | 24);
        meta_w.u8(length as u8);
    } else {
        meta_w.u8(major | 25);
        meta_w.buf(&(length as u16).to_be_bytes());
    }
}

fn encode_node(
    model: &Model,
    node: &CrdtNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    match node {
        CrdtNode::Con(n) => encode_con(n, view_w, meta_w, enc),
        CrdtNode::Val(n) => encode_val(model, n, view_w, meta_w, enc),
        CrdtNode::Obj(n) => encode_obj(model, n, view_w, meta_w, enc),
        CrdtNode::Vec(n) => encode_vec(model, n, view_w, meta_w, enc),
        CrdtNode::Str(n) => encode_str(n, view_w, meta_w, enc),
        CrdtNode::Bin(n) => encode_bin(n, view_w, meta_w, enc),
        CrdtNode::Arr(n) => encode_arr(model, n, view_w, meta_w, enc),
    }
}

fn encode_con(
    node: &ConNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    match &node.val {
        ConValue::Ref(ref_ts) => {
            // Ref: view gets null, meta gets type=1 + ref ts
            write_cbor_null(view_w);
            meta_w.u8(MAJOR_CON | 1);
            ts_logical(meta_w, *ref_ts, enc);
        }
        ConValue::Val(pv) => {
            // Value: view gets the CBOR value, meta gets type=0
            write_cbor_value(view_w, pv);
            meta_w.u8(MAJOR_CON | 0);
        }
    }
}

fn encode_val(
    model: &Model,
    node: &ValNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    meta_w.u8(MAJOR_VAL | 0);
    if let Some(child) = model.index.get(&TsKey::from(node.val)) {
        encode_node(model, child, view_w, meta_w, enc);
    }
}

fn encode_obj(
    model: &Model,
    node: &ObjNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    let n = node.keys.len();
    write_tl(meta_w, MAJOR_OBJ, n);

    let mut sorted_keys: Vec<&String> = node.keys.keys().collect();
    sorted_keys.sort();

    // View: CBOR map header + key strings in sorted order
    write_cbor_map_hdr(view_w, n);
    for key in &sorted_keys {
        write_cbor_str(view_w, key);
    }

    for key in &sorted_keys {
        let child_ts = node.keys[key.as_str()];
        if let Some(child) = model.index.get(&TsKey::from(child_ts)) {
            encode_node(model, child, view_w, meta_w, enc);
        }
    }
}

fn encode_vec(
    model: &Model,
    node: &VecNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    let n = node.elements.len();
    write_tl(meta_w, MAJOR_VEC, n);
    write_cbor_arr_hdr(view_w, n);
    for elem in &node.elements {
        match elem {
            None => {
                write_cbor_null(view_w);
                // undefined/placeholder in meta
                meta_w.u8(0);
            }
            Some(id) => {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node(model, child, view_w, meta_w, enc);
                } else {
                    write_cbor_null(view_w);
                    meta_w.u8(0);
                }
            }
        }
    }
}

fn encode_str(
    node: &StrNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    let n = node.rga.chunk_count();
    write_tl(meta_w, MAJOR_STR, n);

    // View: the concatenated string
    let s = node
        .rga
        .iter_live()
        .filter_map(|c| c.data.as_deref())
        .collect::<String>();
    write_cbor_str(view_w, &s);

    // Meta: for each chunk: id + b1vu56(deleted, span)
    for chunk in node.rga.iter() {
        ts_logical(meta_w, chunk.id, enc);
        meta_w.b1vu56(chunk.deleted as u8, chunk.span);
    }
}

fn encode_bin(
    node: &BinNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    let n = node.rga.chunk_count();
    write_tl(meta_w, MAJOR_BIN, n);

    // View: the concatenated binary
    let bytes: Vec<u8> = node
        .rga
        .iter_live()
        .flat_map(|c| c.data.as_deref().unwrap_or(&[]))
        .copied()
        .collect();
    write_cbor_bin(view_w, &bytes);

    for chunk in node.rga.iter() {
        ts_logical(meta_w, chunk.id, enc);
        meta_w.b1vu56(chunk.deleted as u8, chunk.span);
    }
}

fn encode_arr(
    model: &Model,
    node: &ArrNode,
    view_w: &mut CrdtWriter,
    meta_w: &mut CrdtWriter,
    enc: &mut ClockEncoder,
) {
    ts_logical(meta_w, node.id, enc);
    let n = node.rga.chunk_count();
    write_tl(meta_w, MAJOR_ARR, n);

    // View: array header of live elements
    let live_count = node
        .rga
        .iter_live()
        .filter_map(|c| c.data.as_ref())
        .map(|v| v.len())
        .sum::<usize>();
    write_cbor_arr_hdr(view_w, live_count);

    for chunk in node.rga.iter() {
        ts_logical(meta_w, chunk.id, enc);
        let deleted = chunk.deleted;
        let span = chunk.span;
        meta_w.b1vu56(deleted as u8, span);
        if !deleted {
            let ids = chunk.data.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
            for id in ids {
                if let Some(child) = model.index.get(&TsKey::from(*id)) {
                    encode_node(model, child, view_w, meta_w, enc);
                }
            }
        }
    }
}

// ── CBOR write helpers ─────────────────────────────────────────────────────

fn write_cbor_null(w: &mut CrdtWriter) {
    w.u8(0xF6);
}

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
        PV::Bytes(b) => write_cbor_bin(w, b),
        PV::Array(arr) => {
            write_cbor_arr_hdr(w, arr.len());
            for item in arr {
                write_cbor_value(w, item);
            }
        }
        PV::Object(map) => {
            write_cbor_map_hdr(w, map.len());
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
        PV::Extension(_) | PV::Blob(_) => w.u8(0xF6),
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
    if n <= 23 {
        w.u8(0x20 | n as u8);
    } else if n <= 0xFF {
        w.u8(0x38);
        w.u8(n as u8);
    } else {
        w.u8(0x39);
        w.buf(&(n as u16).to_be_bytes());
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

fn write_cbor_bin(w: &mut CrdtWriter, b: &[u8]) {
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

fn write_cbor_arr_hdr(w: &mut CrdtWriter, n: usize) {
    if n <= 23 {
        w.u8(0x80 | n as u8);
    } else if n <= 0xFF {
        w.u8(0x98);
        w.u8(n as u8);
    } else {
        w.u8(0x99);
        w.buf(&(n as u16).to_be_bytes());
    }
}

fn write_cbor_map_hdr(w: &mut CrdtWriter, n: usize) {
    if n <= 23 {
        w.u8(0xA0 | n as u8);
    } else if n <= 0xFF {
        w.u8(0xB8);
        w.u8(n as u8);
    } else {
        w.u8(0xB9);
        w.buf(&(n as u16).to_be_bytes());
    }
}

// ── Decode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during sidecar binary decode.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("unknown node major type: {0}")]
    UnknownMajor(u8),
    #[error("invalid clock table")]
    InvalidClockTable,
    #[error("format error: {0}")]
    Format(String),
}

/// Decode a sidecar binary document from `(view, meta)` byte arrays.
///
/// `view` is the plain-JSON CBOR stream; `meta` is the CRDT metadata stream.
pub fn decode(view: &[u8], meta: &[u8]) -> Result<Model, DecodeError> {
    if meta.len() < 4 {
        return Err(DecodeError::EndOfInput);
    }
    let mut meta_r = CrdtReader::new(meta);
    let mut view_r = CrdtReader::new(view);

    // Read 4-byte clock-table offset
    let off_bytes = meta_r.buf(4);
    let clock_table_offset =
        u32::from_be_bytes([off_bytes[0], off_bytes[1], off_bytes[2], off_bytes[3]]) as usize;

    let tree_start = meta_r.x;

    // Jump to clock table
    meta_r.x = tree_start + clock_table_offset;

    let n = meta_r.vu57() as usize;
    if n == 0 {
        return Err(DecodeError::InvalidClockTable);
    }
    let first_sid = meta_r.vu57();
    let first_time = meta_r.vu57();
    let mut cd = ClockDecoder::new(first_sid, first_time);
    for _ in 1..n {
        let sid = meta_r.vu57();
        let time = meta_r.vu57();
        cd.push_tuple(sid, time);
    }
    let clock = cd.clock.clone();
    let mut model = Model::new_from_clock(clock);

    // Return to tree start
    meta_r.x = tree_start;

    let root = decode_root(&mut view_r, &mut meta_r, &mut model, &cd)?;
    model.root.val = root;
    Ok(model)
}

fn decode_root(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    if meta_r.x >= meta_r.data.len() {
        return Ok(UNDEFINED_TS);
    }
    let peek = meta_r.data[meta_r.x];
    if peek == 0 {
        meta_r.x += 1;
        Ok(UNDEFINED_TS)
    } else {
        decode_node(view_r, meta_r, model, cd)
    }
}

fn read_ts_logical(meta_r: &mut CrdtReader, cd: &ClockDecoder) -> Result<Ts, DecodeError> {
    let (session_index, time_diff) = meta_r.id();
    cd.decode_id(session_index as u32, time_diff)
        .ok_or_else(|| DecodeError::Format(format!("invalid session index {}", session_index)))
}

fn decode_node(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    let id = read_ts_logical(meta_r, cd)?;
    let octet = meta_r.u8();
    let major = octet >> 5;
    let info = octet & 0x1F;
    let length = if info < 24 {
        info as usize
    } else if info == 24 {
        meta_r.u8() as usize
    } else if info == 25 {
        let b = meta_r.buf(2);
        u16::from_be_bytes([b[0], b[1]]) as usize
    } else {
        meta_r.vu57() as usize
    };

    match major {
        0 => decode_con(view_r, meta_r, model, id, length, cd),
        1 => decode_val(view_r, meta_r, model, id, cd),
        2 => decode_obj(view_r, meta_r, model, id, length, cd),
        3 => decode_vec(view_r, meta_r, model, id, length, cd),
        4 => decode_str(view_r, meta_r, model, id, length, cd),
        5 => decode_bin(view_r, meta_r, model, id, length, cd),
        6 => decode_arr(view_r, meta_r, model, id, length, cd),
        other => Err(DecodeError::UnknownMajor(other)),
    }
}

fn decode_con(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    // length == 0: the CBOR value is in view; length == 1: it's a timestamp ref
    let con_val = if length == 0 {
        let pv = read_cbor_value_sidecar(view_r)?;
        ConValue::Val(pv)
    } else {
        // Ref: view has a null placeholder, meta has the ref timestamp
        let _ = read_cbor_value_sidecar(view_r)?; // consume null placeholder
        let ref_ts = read_ts_logical(meta_r, cd)?;
        ConValue::Ref(ref_ts)
    };

    use crate::json_crdt::nodes::ConNode;
    model
        .index
        .insert(TsKey::from(id), CrdtNode::Con(ConNode::new(id, con_val)));
    Ok(id)
}

fn decode_val(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    let child_id = decode_node(view_r, meta_r, model, cd)?;
    use crate::json_crdt::nodes::ValNode;
    let mut node = ValNode::new(id);
    node.val = child_id;
    model.index.insert(TsKey::from(id), CrdtNode::Val(node));
    Ok(id)
}

fn decode_obj(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::ObjNode;
    // Read view: CBOR map header + key strings (sorted)
    skip_cbor_map_header(view_r)?;
    let mut keys = Vec::new();
    for _ in 0..length {
        let k = read_cbor_str_sidecar(view_r)?;
        keys.push(k);
    }

    let mut node = ObjNode::new(id);
    for key in keys {
        let child_id = decode_node(view_r, meta_r, model, cd)?;
        node.keys.insert(key, child_id);
    }
    model.index.insert(TsKey::from(id), CrdtNode::Obj(node));
    Ok(id)
}

fn decode_vec(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    length: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::VecNode;
    // Skip view array header
    skip_cbor_array_header(view_r)?;

    let mut node = VecNode::new(id);
    for _ in 0..length {
        let peek = meta_r.data[meta_r.x];
        if peek == 0 {
            meta_r.x += 1;
            // Skip null from view
            skip_cbor_value(view_r).map_err(|e| DecodeError::Format(e))?;
            node.elements.push(None);
        } else {
            let child_id = decode_node(view_r, meta_r, model, cd)?;
            node.elements.push(Some(child_id));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Vec(node));
    Ok(id)
}

fn decode_str(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::StrNode;

    // Read the full string from view
    let full_str = read_cbor_str_sidecar(view_r)?;

    let mut node = StrNode::new(id);
    let mut offset = 0usize;

    for _ in 0..count {
        let chunk_id = read_ts_logical(meta_r, cd)?;
        let (deleted_flag, span) = meta_r.b1vu56();
        let deleted = deleted_flag != 0;
        if deleted {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let char_count = span as usize;
            // Extract `char_count` chars from offset
            let text: String = full_str.chars().skip(offset).take(char_count).collect();
            offset += char_count;
            node.rga.push_chunk(Chunk::new(chunk_id, span, text));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Str(node));
    Ok(id)
}

fn decode_bin(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::BinNode;

    // Read binary from view
    let full_bin = read_cbor_bin_sidecar(view_r)?;

    let mut node = BinNode::new(id);
    let mut offset = 0usize;

    for _ in 0..count {
        let chunk_id = read_ts_logical(meta_r, cd)?;
        let (deleted_flag, span) = meta_r.b1vu56();
        let deleted = deleted_flag != 0;
        if deleted {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let len = span as usize;
            let data = full_bin[offset..offset + len].to_vec();
            offset += len;
            node.rga.push_chunk(Chunk::new(chunk_id, span, data));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Bin(node));
    Ok(id)
}

fn decode_arr(
    view_r: &mut CrdtReader,
    meta_r: &mut CrdtReader,
    model: &mut Model,
    id: Ts,
    count: usize,
    cd: &ClockDecoder,
) -> Result<Ts, DecodeError> {
    use crate::json_crdt::nodes::rga::Chunk;
    use crate::json_crdt::nodes::ArrNode;

    // Skip view array header
    skip_cbor_array_header(view_r)?;

    let mut node = ArrNode::new(id);
    for _ in 0..count {
        let chunk_id = read_ts_logical(meta_r, cd)?;
        let (deleted_flag, span) = meta_r.b1vu56();
        let deleted = deleted_flag != 0;
        if deleted {
            node.rga.push_chunk(Chunk::new_deleted(chunk_id, span));
        } else {
            let mut ids = Vec::new();
            for _ in 0..span {
                let child_id = decode_node(view_r, meta_r, model, cd)?;
                ids.push(child_id);
            }
            node.rga.push_chunk(Chunk::new(chunk_id, span, ids));
        }
    }
    model.index.insert(TsKey::from(id), CrdtNode::Arr(node));
    Ok(id)
}

// ── CBOR sidecar reader helpers ────────────────────────────────────────────

fn read_cbor_value_sidecar(r: &mut CrdtReader) -> Result<PackValue, DecodeError> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;
    match major {
        0 => {
            let n = read_cbor_arg(r, info)?;
            Ok(PackValue::Integer(n as i64))
        }
        1 => {
            let n = read_cbor_arg(r, info)?;
            Ok(PackValue::Integer(-1 - n as i64))
        }
        2 => {
            let len = read_cbor_arg(r, info)? as usize;
            Ok(PackValue::Bytes(r.buf(len).to_vec()))
        }
        3 => {
            let len = read_cbor_arg(r, info)? as usize;
            Ok(PackValue::Str(r.utf8(len).to_string()))
        }
        4 => {
            let len = read_cbor_arg(r, info)? as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(read_cbor_value_sidecar(r)?);
            }
            Ok(PackValue::Array(items))
        }
        5 => {
            let len = read_cbor_arg(r, info)? as usize;
            let mut map = Vec::with_capacity(len);
            for _ in 0..len {
                let k = match read_cbor_value_sidecar(r)? {
                    PackValue::Str(s) => s,
                    _ => String::new(),
                };
                let v = read_cbor_value_sidecar(r)?;
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
                Ok(PackValue::Float(f64::from_be_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ])))
            }
            _ => Ok(PackValue::Null),
        },
        _ => Err(DecodeError::Format(format!("unknown CBOR major {}", major))),
    }
}

fn read_cbor_arg(r: &mut CrdtReader, info: u8) -> Result<u64, DecodeError> {
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
        _ => Err(DecodeError::Format(format!("unsupported info {}", info))),
    }
}

fn read_cbor_str_sidecar(r: &mut CrdtReader) -> Result<String, DecodeError> {
    match read_cbor_value_sidecar(r)? {
        PackValue::Str(s) => Ok(s),
        _ => Err(DecodeError::Format("expected string".into())),
    }
}

fn read_cbor_bin_sidecar(r: &mut CrdtReader) -> Result<Vec<u8>, DecodeError> {
    match read_cbor_value_sidecar(r)? {
        PackValue::Bytes(b) => Ok(b),
        _ => Err(DecodeError::Format("expected binary".into())),
    }
}

fn skip_cbor_value(r: &mut CrdtReader) -> Result<(), String> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;
    let arg = skip_cbor_arg(r, info)?;
    match major {
        0 | 1 | 7 => Ok(()),
        2 | 3 => {
            r.x += arg as usize;
            Ok(())
        }
        4 => {
            for _ in 0..arg {
                skip_cbor_value(r)?;
            }
            Ok(())
        }
        5 => {
            for _ in 0..(arg * 2) {
                skip_cbor_value(r)?;
            }
            Ok(())
        }
        _ => Err(format!("unknown major {}", major)),
    }
}

fn skip_cbor_arg(r: &mut CrdtReader, info: u8) -> Result<u64, String> {
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
        _ => Err(format!("unsupported info {}", info)),
    }
}

fn skip_cbor_map_header(r: &mut CrdtReader) -> Result<u64, DecodeError> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;
    if major != 5 {
        return Err(DecodeError::Format(format!(
            "expected CBOR map, got major {}",
            major
        )));
    }
    read_cbor_arg(r, info)
}

fn skip_cbor_array_header(r: &mut CrdtReader) -> Result<u64, DecodeError> {
    let byte = r.u8();
    let major = byte >> 5;
    let info = byte & 0x1F;
    if major != 4 {
        return Err(DecodeError::Format(format!(
            "expected CBOR array, got major {}",
            major
        )));
    }
    read_cbor_arg(r, info)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;

    fn sid() -> u64 {
        555666
    }

    #[test]
    fn encode_produces_two_parts() {
        let model = Model::new(sid());
        let (view, meta) = encode(&model);
        // meta must have at least 4 bytes (clock offset) + root + clock table
        assert!(meta.len() >= 4);
        // view should be empty or minimal for an empty model
        let _ = view;
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
            data: "sidecar".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view_val = model.view();
        let (view_bytes, meta_bytes) = encode(&model);
        let decoded = decode(&view_bytes, &meta_bytes).expect("decode");
        assert_eq!(decoded.view(), view_val);
    }

    #[test]
    fn roundtrip_con_number() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(55)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: crate::json_crdt::constants::ORIGIN,
            val: ts(s, 1),
        });
        let view_val = model.view();
        let (view_bytes, meta_bytes) = encode(&model);
        let decoded = decode(&view_bytes, &meta_bytes).expect("decode");
        assert_eq!(decoded.view(), view_val);
    }
}
