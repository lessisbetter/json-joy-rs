use std::collections::{BTreeMap, HashMap};

use ciborium::value::Value as CborValue;
use serde_json::Value;
use thiserror::Error;

use crate::crdt_binary::{read_b1vu56, read_vu57, write_b1vu56, write_vu57, LogicalClockBase};
use crate::model::ModelError;
use crate::model_runtime::types::{ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};
use crate::model_runtime::RuntimeModel;

#[derive(Debug, Error)]
pub enum SidecarBinaryCodecError {
    #[error("invalid sidecar payload")]
    InvalidPayload,
    #[error("model runtime failure: {0}")]
    Model(#[from] ModelError),
}

pub fn encode_model_binary_to_sidecar(
    model_binary: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), SidecarBinaryCodecError> {
    let runtime = RuntimeModel::from_model_binary(model_binary)?;
    if runtime.server_clock_time.is_some() || runtime.clock_table.is_empty() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let mut ctx = ClockEncCtx::new(&runtime.clock_table)?;
    let mut meta = vec![0, 0, 0, 0];
    let view = match runtime.root {
        Some(root) if root.sid != 0 => {
            let view_cbor = encode_node_view(root, &runtime, &mut ctx, &mut meta)?;
            let mut encoded = Vec::new();
            write_cbor_value_json_pack(&mut encoded, &view_cbor)?;
            encoded
        }
        _ => {
            meta.push(0);
            Vec::new()
        }
    };

    let table_offset = (meta.len() - 4) as u32;
    meta[0] = ((table_offset >> 24) & 0xff) as u8;
    meta[1] = ((table_offset >> 16) & 0xff) as u8;
    meta[2] = ((table_offset >> 8) & 0xff) as u8;
    meta[3] = (table_offset & 0xff) as u8;

    write_vu57(&mut meta, ctx.table.len() as u64);
    for c in &ctx.table {
        write_vu57(&mut meta, c.sid);
        write_vu57(&mut meta, c.time);
    }

    Ok((view, meta))
}

pub fn decode_sidecar_to_model_binary(
    view_binary: &[u8],
    meta_binary: &[u8],
) -> Result<Vec<u8>, SidecarBinaryCodecError> {
    if meta_binary.len() < 4 {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }
    let offset = u32::from_be_bytes([
        meta_binary[0],
        meta_binary[1],
        meta_binary[2],
        meta_binary[3],
    ]) as usize;
    if 4 + offset > meta_binary.len() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let mut table_pos = 4 + offset;
    let len = read_vu57(meta_binary, &mut table_pos)
        .ok_or(SidecarBinaryCodecError::InvalidPayload)? as usize;
    if len == 0 {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }
    let mut table = Vec::with_capacity(len);
    for _ in 0..len {
        let sid = read_vu57(meta_binary, &mut table_pos)
            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        let time = read_vu57(meta_binary, &mut table_pos)
            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        table.push(LogicalClockBase { sid, time });
    }

    let mut dec = MetaCursor::new(&meta_binary[4..4 + offset]);
    let mut nodes = HashMap::new();
    let root = if dec.peek().ok_or(SidecarBinaryCodecError::InvalidPayload)? == 0 {
        dec.u8()
            .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
        None
    } else {
        let view: CborValue = ciborium::de::from_reader(view_binary)
            .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
        Some(decode_node_from_sidecar(
            &view, &mut dec, &table, &mut nodes,
        )?)
    };
    if !dec.is_eof() {
        return Err(SidecarBinaryCodecError::InvalidPayload);
    }

    let runtime = RuntimeModel {
        nodes,
        root,
        clock: Default::default(),
        fallback_view: Value::Null,
        infer_empty_object_root: false,
        clock_table: table,
        server_clock_time: None,
    };
    runtime
        .to_model_binary_like()
        .map_err(SidecarBinaryCodecError::from)
}

struct ClockEncCtx {
    table: Vec<LogicalClockBase>,
    by_sid: HashMap<u64, usize>,
    local_base: u64,
}

impl ClockEncCtx {
    fn new(clock_table: &[LogicalClockBase]) -> Result<Self, SidecarBinaryCodecError> {
        let local = clock_table
            .first()
            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        let mut table = Vec::with_capacity(clock_table.len());
        let mut by_sid = HashMap::new();
        for (idx, c) in clock_table.iter().enumerate() {
            table.push(*c);
            by_sid.insert(c.sid, idx + 1);
        }
        Ok(Self {
            table,
            by_sid,
            local_base: local.time,
        })
    }

    fn append(&mut self, id: Id, out: &mut Vec<u8>) -> Result<(), SidecarBinaryCodecError> {
        if id.sid == 0 {
            write_sidecar_id(out, 0, id.time);
            return Ok(());
        }
        let idx = match self.by_sid.get(&id.sid) {
            Some(v) => *v,
            None => {
                self.table.push(LogicalClockBase {
                    sid: id.sid,
                    time: self.local_base,
                });
                let n = self.table.len();
                self.by_sid.insert(id.sid, n);
                n
            }
        };
        let base = self.table[idx - 1].time;
        let diff = base
            .checked_sub(id.time)
            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        write_sidecar_id(out, idx as u64, diff);
        Ok(())
    }
}

fn write_sidecar_id(out: &mut Vec<u8>, session_index: u64, time_diff: u64) {
    if session_index <= 0b111 && time_diff <= 0b1111 {
        out.push(((session_index as u8) << 4) | (time_diff as u8));
    } else {
        write_b1vu56(out, 1, session_index);
        write_vu57(out, time_diff);
    }
}

fn decode_sidecar_id(
    cur: &mut MetaCursor<'_>,
    table: &[LogicalClockBase],
) -> Result<Id, SidecarBinaryCodecError> {
    let first = cur
        .u8()
        .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
    let (session_index, time_diff) = if first <= 0x7f {
        ((first >> 4) as u64, (first & 0x0f) as u64)
    } else {
        cur.pos -= 1;
        let (flag, x) =
            read_b1vu56(cur.data, &mut cur.pos).ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        if flag != 1 {
            return Err(SidecarBinaryCodecError::InvalidPayload);
        }
        let y = read_vu57(cur.data, &mut cur.pos).ok_or(SidecarBinaryCodecError::InvalidPayload)?;
        (x, y)
    };

    if session_index == 0 {
        return Ok(Id {
            sid: 0,
            time: time_diff,
        });
    }
    let base = table
        .get(session_index as usize - 1)
        .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
    let time = base
        .time
        .checked_sub(time_diff)
        .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
    Ok(Id {
        sid: base.sid,
        time,
    })
}

fn write_type_len(out: &mut Vec<u8>, major: u8, len: u64) {
    if len < 24 {
        out.push((major << 5) | (len as u8));
    } else if len <= 0xff {
        out.push((major << 5) | 24);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push((major << 5) | 25);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    } else {
        out.push((major << 5) | 26);
        out.push(((len >> 24) & 0xff) as u8);
        out.push(((len >> 16) & 0xff) as u8);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    }
}

fn write_cbor_uint(out: &mut Vec<u8>, major: u8, value: u64) {
    if value <= 23 {
        out.push((major << 5) | (value as u8));
    } else if value <= 0xff {
        out.push((major << 5) | 24);
        out.push(value as u8);
    } else if value <= 0xffff {
        out.push((major << 5) | 25);
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= 0xffff_ffff {
        out.push((major << 5) | 26);
        out.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        out.push((major << 5) | 27);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn write_json_pack_str(out: &mut Vec<u8>, s: &str) {
    let max_size = s.len().saturating_mul(4);
    let utf8 = s.as_bytes();
    let len = utf8.len();
    if max_size <= 23 {
        out.push(0x60 | (len as u8));
    } else if max_size <= 0xff {
        out.push(0x78);
        out.push(len as u8);
    } else if max_size <= 0xffff {
        out.push(0x79);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(0x7a);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
    out.extend_from_slice(utf8);
}

fn write_cbor_value_json_pack(
    out: &mut Vec<u8>,
    v: &CborValue,
) -> Result<(), SidecarBinaryCodecError> {
    match v {
        CborValue::Null => out.push(0xf6),
        CborValue::Bool(false) => out.push(0xf4),
        CborValue::Bool(true) => out.push(0xf5),
        CborValue::Integer(i) => {
            if let Ok(u) = u64::try_from(*i) {
                write_cbor_uint(out, 0, u);
            } else if let Ok(s) = i64::try_from(*i) {
                write_cbor_uint(out, 1, (-1 - s) as u64);
            } else {
                return Err(SidecarBinaryCodecError::InvalidPayload);
            }
        }
        CborValue::Float(f) => {
            let f32v = *f as f32;
            if (f32v as f64) == *f {
                out.push(0xfa);
                out.extend_from_slice(&f32v.to_be_bytes());
            } else {
                out.push(0xfb);
                out.extend_from_slice(&f.to_be_bytes());
            }
        }
        CborValue::Text(s) => write_json_pack_str(out, s),
        CborValue::Bytes(b) => {
            write_cbor_uint(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        CborValue::Array(arr) => {
            write_cbor_uint(out, 4, arr.len() as u64);
            for item in arr {
                write_cbor_value_json_pack(out, item)?;
            }
        }
        CborValue::Map(map) => {
            write_cbor_uint(out, 5, map.len() as u64);
            for (k, item) in map {
                write_cbor_value_json_pack(out, k)?;
                write_cbor_value_json_pack(out, item)?;
            }
        }
        CborValue::Tag(tag, inner) => {
            write_cbor_uint(out, 6, *tag);
            write_cbor_value_json_pack(out, inner)?;
        }
        _ => return Err(SidecarBinaryCodecError::InvalidPayload),
    }
    Ok(())
}

fn cbor_from_json(v: &Value) -> CborValue {
    json_joy_json_pack::json_to_cbor(v)
}

fn json_from_cbor(v: &CborValue) -> Result<Value, SidecarBinaryCodecError> {
    json_joy_json_pack::cbor_to_json(v).map_err(|_| SidecarBinaryCodecError::InvalidPayload)
}

fn encode_node_view(
    id: Id,
    runtime: &RuntimeModel,
    clock: &mut ClockEncCtx,
    meta: &mut Vec<u8>,
) -> Result<CborValue, SidecarBinaryCodecError> {
    let node = runtime
        .nodes
        .get(&id)
        .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
    clock.append(id, meta)?;
    Ok(match node {
        RuntimeNode::Con(ConCell::Json(v)) => {
            meta.push(0);
            cbor_from_json(v)
        }
        RuntimeNode::Con(ConCell::Ref(ref_id)) => {
            meta.push(1);
            clock.append(*ref_id, meta)?;
            CborValue::Null
        }
        RuntimeNode::Con(ConCell::Undef) => {
            meta.push(0);
            CborValue::Null
        }
        RuntimeNode::Val(child) => {
            meta.push(0b0010_0000);
            encode_node_view(*child, runtime, clock, meta)?
        }
        RuntimeNode::Obj(entries) => {
            let mut sorted: Vec<(&str, Id)> =
                entries.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            write_type_len(meta, 2, sorted.len() as u64);
            let mut map = Vec::with_capacity(sorted.len());
            for (k, child) in sorted {
                let child_view = encode_node_view(child, runtime, clock, meta)?;
                map.push((CborValue::Text(k.to_string()), child_view));
            }
            CborValue::Map(map)
        }
        RuntimeNode::Vec(elements) => {
            let len = elements.keys().max().map(|v| v + 1).unwrap_or(0);
            write_type_len(meta, 3, len);
            let mut arr = Vec::with_capacity(len as usize);
            for i in 0..len {
                if let Some(child) = elements.get(&i) {
                    arr.push(encode_node_view(*child, runtime, clock, meta)?);
                } else {
                    // Missing vec slots are represented as undefined in upstream.
                    // For JSON-transport side this serializes as null.
                    arr.push(CborValue::Null);
                }
            }
            CborValue::Array(arr)
        }
        RuntimeNode::Str(atoms) => {
            let chunks = group_str_chunks(atoms);
            write_type_len(meta, 4, chunks.len() as u64);
            for ch in chunks {
                clock.append(ch.id, meta)?;
                write_b1vu56(meta, if ch.text.is_some() { 0 } else { 1 }, ch.span);
            }
            let mut s = String::new();
            for atom in atoms {
                if let Some(ch) = atom.ch {
                    s.push(ch);
                }
            }
            CborValue::Text(s)
        }
        RuntimeNode::Bin(atoms) => {
            let chunks = group_bin_chunks(atoms);
            write_type_len(meta, 5, chunks.len() as u64);
            for ch in chunks {
                clock.append(ch.id, meta)?;
                write_b1vu56(meta, if ch.bytes.is_some() { 0 } else { 1 }, ch.span);
            }
            let mut b = Vec::new();
            for atom in atoms {
                if let Some(x) = atom.byte {
                    b.push(x);
                }
            }
            CborValue::Bytes(b)
        }
        RuntimeNode::Arr(atoms) => {
            let chunks = group_arr_chunks(atoms);
            write_type_len(meta, 6, chunks.len() as u64);
            let mut values = Vec::new();
            for ch in chunks {
                clock.append(ch.id, meta)?;
                match ch.values {
                    Some(ids) => {
                        write_b1vu56(meta, 0, ch.span);
                        for child in ids {
                            values.push(encode_node_view(child, runtime, clock, meta)?);
                        }
                    }
                    None => {
                        write_b1vu56(meta, 1, ch.span);
                    }
                }
            }
            CborValue::Array(values)
        }
    })
}

fn decode_node_from_sidecar(
    view: &CborValue,
    meta: &mut MetaCursor<'_>,
    table: &[LogicalClockBase],
    nodes: &mut HashMap<Id, RuntimeNode>,
) -> Result<Id, SidecarBinaryCodecError> {
    let id = decode_sidecar_id(meta, table)?;
    let octet = meta
        .u8()
        .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?;
    let major = octet >> 5;
    let len = meta.read_len(octet & 0x1f)?;
    let node = match major {
        0 => {
            if len == 0 {
                RuntimeNode::Con(ConCell::Json(json_from_cbor(view)?))
            } else {
                let ref_id = decode_sidecar_id(meta, table)?;
                RuntimeNode::Con(ConCell::Ref(ref_id))
            }
        }
        1 => {
            let child = decode_node_from_sidecar(view, meta, table, nodes)?;
            RuntimeNode::Val(child)
        }
        2 => {
            let map = match view {
                CborValue::Map(m) => m,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            if map.len() != len as usize {
                return Err(SidecarBinaryCodecError::InvalidPayload);
            }
            let mut keys: Vec<String> = map
                .iter()
                .map(|(k, _)| match k {
                    CborValue::Text(s) => Ok(s.clone()),
                    _ => Err(SidecarBinaryCodecError::InvalidPayload),
                })
                .collect::<Result<_, _>>()?;
            keys.sort();
            let mut entries = Vec::with_capacity(keys.len());
            for k in keys {
                let child_view = map
                    .iter()
                    .find_map(|(kk, vv)| match kk {
                        CborValue::Text(s) if s == &k => Some(vv),
                        _ => None,
                    })
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                entries.push((k, child));
            }
            RuntimeNode::Obj(entries)
        }
        3 => {
            let arr = match view {
                CborValue::Array(a) => a,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            if arr.len() != len as usize {
                return Err(SidecarBinaryCodecError::InvalidPayload);
            }
            let mut elements = BTreeMap::new();
            for (idx, child_view) in arr.iter().enumerate() {
                let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                if child.sid != 0 {
                    elements.insert(idx as u64, child);
                }
            }
            RuntimeNode::Vec(elements)
        }
        4 => {
            let s = match view {
                CborValue::Text(s) => s,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut chars = s.chars();
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(StrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            ch: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let ch = chars
                            .next()
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        atoms.push(StrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            ch: Some(ch),
                        });
                    }
                }
            }
            RuntimeNode::Str(atoms)
        }
        5 => {
            let bytes = match view {
                CborValue::Bytes(b) => b,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut byte_pos = 0usize;
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            byte: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let b = *bytes
                            .get(byte_pos)
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        byte_pos += 1;
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            byte: Some(b),
                        });
                    }
                }
            }
            RuntimeNode::Bin(atoms)
        }
        6 => {
            let arr = match view {
                CborValue::Array(a) => a,
                _ => return Err(SidecarBinaryCodecError::InvalidPayload),
            };
            let mut view_idx = 0usize;
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_sidecar_id(meta, table)?;
                let (deleted, span) = read_b1vu56(meta.data, &mut meta.pos)
                    .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let child_view = arr
                            .get(view_idx)
                            .ok_or(SidecarBinaryCodecError::InvalidPayload)?;
                        view_idx += 1;
                        let child = decode_node_from_sidecar(child_view, meta, table, nodes)?;
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: Some(child),
                        });
                    }
                }
            }
            RuntimeNode::Arr(atoms)
        }
        _ => return Err(SidecarBinaryCodecError::InvalidPayload),
    };

    nodes.insert(id, node);
    Ok(id)
}

struct MetaCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> MetaCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, ()> {
        let b = *self.data.get(self.pos).ok_or(())?;
        self.pos += 1;
        Ok(b)
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn read_len(&mut self, minor: u8) -> Result<u64, SidecarBinaryCodecError> {
        Ok(match minor {
            0..=23 => minor as u64,
            24 => self
                .u8()
                .map_err(|_| SidecarBinaryCodecError::InvalidPayload)? as u64,
            25 => {
                let a = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                let b = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                (a << 8) | b
            }
            26 => {
                let a = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                let b = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                let c = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                let d = self
                    .u8()
                    .map_err(|_| SidecarBinaryCodecError::InvalidPayload)?
                    as u64;
                (a << 24) | (b << 16) | (c << 8) | d
            }
            _ => return Err(SidecarBinaryCodecError::InvalidPayload),
        })
    }

    fn is_eof(&self) -> bool {
        self.pos == self.data.len()
    }
}

struct StrChunk {
    id: Id,
    span: u64,
    text: Option<String>,
}
struct BinChunk {
    id: Id,
    span: u64,
    bytes: Option<Vec<u8>>,
}
struct ArrChunk {
    id: Id,
    span: u64,
    values: Option<Vec<Id>>,
}

fn group_str_chunks(atoms: &[StrAtom]) -> Vec<StrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].ch.is_some() == atoms[j].ch.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.ch.is_some() {
            let mut s = String::new();
            for a in &atoms[i..j] {
                if let Some(ch) = a.ch {
                    s.push(ch);
                }
            }
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: Some(s),
            });
        } else {
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: None,
            });
        }
        i = j;
    }
    out
}

fn group_bin_chunks(atoms: &[BinAtom]) -> Vec<BinChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].byte.is_some() == atoms[j].byte.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.byte.is_some() {
            let mut bytes = Vec::new();
            for a in &atoms[i..j] {
                if let Some(b) = a.byte {
                    bytes.push(b);
                }
            }
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: Some(bytes),
            });
        } else {
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: None,
            });
        }
        i = j;
    }
    out
}

fn group_arr_chunks(atoms: &[ArrAtom]) -> Vec<ArrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].value.is_some() == atoms[j].value.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.value.is_some() {
            let mut values = Vec::new();
            for a in &atoms[i..j] {
                if let Some(v) = a.value {
                    values.push(v);
                }
            }
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: Some(values),
            });
        } else {
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: None,
            });
        }
        i = j;
    }
    out
}
